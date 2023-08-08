use std::fmt::{self, Formatter};
use std::marker::ConstParamTy;
use std::matches;
use std::ops::Index;
use std::sync::Arc;
use std::{fmt::Display, fs::read_to_string};

use async_trait::async_trait;
use discord::escape_string;
use discord::message::Field;
use discord::{
    interaction::{Interaction, MessageComponent},
    resource::Snowflake,
    user::User,
};
use rand::seq::{IteratorRandom, SliceRandom};
use rand::{thread_rng, Rng};
use serde::Deserialize;

use crate::game::widget::{ChoiceGrid, Event, Widget};
use crate::game::{widget::SelectionGrid, Flow, Game, GameMessage, GameUI, Logic};

use self::setup::Setup;

// mod write;
// mod read;
mod setup;

#[derive(Deserialize)]
#[serde(untagged)]
pub enum CardData {
    Raw(String),
    Full { text: String, pick: usize },
}

impl CardData {
    fn blanks(&self) -> usize {
        match *self {
            CardData::Raw(ref s) => s.chars().filter(|&c| c == '_').count().min(1),
            CardData::Full { pick, .. } => pick,
        }
    }
    fn extra_blanks(&self) -> usize {
        match *self {
            CardData::Raw(ref s) => {
                if s.contains('_') {
                    0
                } else {
                    1
                }
            }
            CardData::Full { ref text, pick } => pick - text.chars().filter(|&c| c == '_').count(),
        }
    }
}

impl Display for CardData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CardData::Raw(text) => text.fmt(f),
            CardData::Full { text, .. } => text.fmt(f),
        }
    }
}

#[derive(Deserialize)]
pub struct PackData {
    black: Vec<CardData>,
    white: Vec<CardData>,
}

#[derive(ConstParamTy, PartialEq, Eq, Clone, Copy)]
pub enum CardType {
    White,
    Black,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Card<const TYPE: CardType> {
    pack: u32,
    card: u32,
}

impl Card<{ CardType::Black }> {
    pub fn fill(
        self,
        packs: &Packs,
        mut white: impl Iterator<Item = Option<Card<{ CardType::White }>>>,
    ) -> String {
        let prompt = escape_string(&packs[self].to_string().replace("\\n", "\n "));

        let mut prompt_slice = prompt.as_str();
        let mut filled = String::with_capacity(prompt.len());

        while let Some(pos) = prompt_slice.find('_') {
            match white.next() {
                Some(Some(c)) => {
                    filled.push_str(&prompt_slice[..pos - 1]);

                    // TODO: recursive
                    filled.push_str(format!("``{}``", packs[c]).as_str());
                }
                _ => {
                    filled.push_str(&prompt_slice[..pos + 1]);
                }
            }
            prompt_slice = &prompt_slice[pos + 1..];
        }
        filled.push_str(prompt_slice);

        let extra_cards = packs[self].extra_blanks();
        for _ in 0..extra_cards {
            if let Some(Some(c)) = white.next() {
                // TODO: recursive
                filled.push_str(format!(" ``{}``", packs[c]).as_str());
            }
        }

        filled
    }
    pub fn is_filled(
        self,
        packs: &Packs,
        white: impl Iterator<Item = Option<Card<{ CardType::White }>>>,
    ) -> bool {
        let mut blanks = packs[self].blanks();
        let mut cards = 0;

        for card in white {
            match card {
                Some(card) => {
                    // NOTE: this already accounts for recursiveness
                    blanks += packs[card].blanks();
                    cards += 1;
                }
                None => return false,
            }
        }

        cards >= blanks
    }
}

pub type Pack = Arc<(String, PackData)>;
pub struct Packs(Vec<Pack>);

impl Index<Card<{ CardType::Black }>> for Packs {
    type Output = CardData;

    fn index(&self, index: Card<{ CardType::Black }>) -> &Self::Output {
        &self.0[index.pack as usize].1.black[index.card as usize]
    }
}

impl Index<Card<{ CardType::White }>> for Packs {
    type Output = CardData;

    fn index(&self, index: Card<{ CardType::White }>) -> &Self::Output {
        &self.0[index.pack as usize].1.white[index.card as usize]
    }
}

#[derive(PartialEq, Eq, Clone)]
pub enum PlayerKind {
    User(Snowflake<User>),
    Rando(usize),
}

impl Display for PlayerKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PlayerKind::User(u) => u.fmt(f),
            PlayerKind::Rando(i) => write!(f, "`Rando Cardrissian #{}`", i + 1),
        }
    }
}

pub struct Player {
    pub kind: PlayerKind,
    pub points: i32,
    pub hand: Vec<Card<{ CardType::White }>>,
    pub selected: SelectionGrid,
}

impl Player {
    pub fn new(kind: PlayerKind, cards: usize) -> Self {
        Self {
            kind,
            points: 0,
            hand: Vec::new(),
            selected: SelectionGrid {
                count: cards,
                selected: Vec::new(),
                disable_unselected: false,
            },
        }
    }
    pub fn selected(&self) -> impl Iterator<Item = Option<Card<{ CardType::White }>>> + '_ {
        self.selected
            .selected
            .iter()
            .copied()
            .map(|i| i.map(|i| self.hand[i]))
    }
    pub fn draw(
        &mut self,
        packs: &mut Packs,
        max: usize,
        prompt: Card<{ CardType::Black }>,
    ) -> bool {
        // remove selected cards
        let mut selected = std::mem::replace(&mut self.selected.selected, Vec::new());
        selected.sort_unstable_by(|a, b| b.cmp(a));
        for index in selected {
            if let Some(index) = index {
                self.hand.remove(index);
            }
        }
        // draw new cards
        for _ in 0..max - self.hand.len() {
            self.hand.push(match packs.draw_white() {
                Some(c) => c,
                None => return false,
            });
        }
        // if rando, give answer immediately
        if matches!(self.kind, PlayerKind::Rando(_)) {
            fn choose(raw: &mut Vec<usize>) -> Option<usize> {
                let i = (0..raw.len()).choose(&mut thread_rng())?;
                Some(raw.swap_remove(i))
            }

            let mut indices: Vec<_> = (0..max).collect();
            while !prompt.is_filled(packs, self.selected()) {
                self.selected
                    .selected
                    .push(Some(match choose(&mut indices) {
                        Some(i) => i,
                        None => return false,
                    }))
            }
            // registers this as completed
            self.selected.disable_unselected = true;
        } else {
            self.selected.disable_unselected = false;
        }
        true
    }
}

impl Packs {
    pub fn draw_black(&mut self) -> Option<Card<{ CardType::Black }>> {
        let start_indices = self
            .0
            .iter()
            .scan(0, |acc, p| {
                let old = *acc;
                *acc += p.1.black.len();
                Some(old)
            })
            .collect::<Vec<_>>();

        let total = *start_indices.last()? + self.0.last()?.1.black.len();
        if total == 0 {
            return None;
        }

        let mut rng = rand::thread_rng();
        let random = rng.gen_range(0..total);

        let (pack, start_index) = start_indices
            .into_iter()
            .enumerate()
            .find(|&(_, t)| random >= t)
            .unwrap();
        let card = random - start_index;

        Some(Card {
            pack: pack as u32,
            card: card as u32,
        })
    }
    pub fn draw_white(&mut self) -> Option<Card<{ CardType::White }>> {
        let start_indices = self
            .0
            .iter()
            .scan(0, |acc, p| {
                let old = *acc;
                *acc += p.1.white.len();
                Some(old)
            })
            .collect::<Vec<_>>();

        let total = *start_indices.last()? + self.0.last()?.1.white.len();
        if total == 0 {
            return None;
        }

        let mut rng = rand::thread_rng();
        let random = rng.gen_range(0..total);

        let (pack, start_index) = start_indices
            .into_iter()
            .enumerate()
            .find(|&(_, t)| random >= t)
            .unwrap();
        let card = random - start_index;

        Some(Card {
            pack: pack as u32,
            card: card as u32,
        })
    }
}

pub enum CAH {
    Setup(Setup),
    Write(Ingame),
    Read(Ingame),
}

pub struct Ingame {
    pub packs: Packs,
    pub cards: usize,
    pub points: i32,
    pub players: Vec<Player>,

    pub prompt: Card<{ CardType::Black }>,
    pub czar: PlayerKind,
}

#[async_trait]
impl Logic for CAH {
    type Return = ();
    async fn logic(&mut self, ui: &mut GameUI, i: Interaction<MessageComponent>) -> Flow<()> {
        match self {
            CAH::Setup(s) => {
                let (msg, done) = (&mut *s).render(Event::component(&i))?;
                if !done {
                    ui.update(i, msg).await?;
                    return Flow::Continue;
                }

                let players: Vec<_> = s.players().collect();
                let packs = Packs(
                    s.packs
                        .0
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| s.selected_packs.contains(i))
                        .map(|(_, p)| p)
                        .cloned()
                        .collect(),
                );

                if players.len() < 2 {
                    ui.reply(
                        i,
                        GameMessage::new(vec![Field::new("Error", "not enough players")], vec![]),
                    )
                    .await?;
                    return Flow::Continue;
                }

                let czar = match players.iter().find(|p| matches!(p, PlayerKind::User(_))) {
                    Some(p) => p.clone(),
                    None => {
                        ui.reply(
                            i,
                            GameMessage::new(vec![Field::new("Error", "I know you want to witness the AI uprising, but you can't play a game with only Rando Cardrissian")], vec![]),
                        )
                        .await?;
                        return Flow::Continue;
                    }
                };

                let mut players = players
                    .into_iter()
                    .map(|p| Player::new(p, s.cards as usize))
                    .collect::<Vec<_>>();

                let prompt = match s.packs.draw_black() {
                    Some(c) => c,
                    None => {
                        ui.reply(
                            i,
                            GameMessage::new(
                                vec![Field::new(
                                    "Error",
                                    "selected packs do not have any black cards",
                                )],
                                vec![],
                            ),
                        )
                        .await?;
                        return Flow::Continue;
                    }
                };

                for player in players.iter_mut() {
                    if !player.draw(&mut s.packs, s.cards as usize, prompt) {
                        ui.reply(
                            i,
                            GameMessage::new(
                                vec![Field::new(
                                    "Error",
                                    "selected packs do not have enough white cards to start",
                                )],
                                vec![],
                            ),
                        )
                        .await?;
                        return Flow::Continue;
                    }
                }

                // start game!
                ui.delete_replies().await?;

                let ingame = Ingame {
                    packs,
                    cards: s.cards as usize,
                    points: s.points,
                    players,
                    prompt,
                    czar,
                };

                if ingame
                    .players
                    .iter()
                    .filter(|p| matches!(p.kind, PlayerKind::User(_)))
                    .count()
                    == 1
                {
                    // only randos immediately go to Read
                    // ui.update(i, read.render()).await?;
                    *self = CAH::Read(ingame);
                } else {
                    // ui.update(i, write.render()).await?;
                    *self = CAH::Write(ingame);
                }
                Flow::Continue
            }
            CAH::Write(_) => todo!(),
            CAH::Read(_) => todo!(),
        }

        // match self {
        //     CAH::Write(d) => {
        //         d.logic(ui, i).await?;

        //         let mut shuffle = (0..d.data.players.len() - 1).collect::<Vec<_>>();
        //         shuffle.shuffle(&mut rand::thread_rng());
        //         let read = read::Read {
        //             choice: ChoiceGrid { shuffle },
        //             data: std::mem::replace(&mut d.data, DATA_DUMMY),
        //         };
        //         ui.delete_replies().await?;
        //         ui.edit(ui.base_message_id(), read.render()).await?;

        //         *self = CAH::Read(read);
        //         Flow::Continue
        //     }
        //     CAH::Read(d) => {
        //         let i = d.logic(ui, i).await?;

        //         // new prompt
        //         d.data.prompt = match d.data.options.packs.draw_black() {
        //             Some(c) => c,
        //             None => todo!(),
        //         };

        //         // draw cards
        //         for player in d.data.players.iter_mut() {
        //             if !player.draw(
        //                 &mut d.data.options.packs,
        //                 d.data.options.cards,
        //                 d.data.prompt,
        //             ) {
        //                 todo!();
        //             }
        //         }

        //         // new czar
        //         let czar = d
        //             .data
        //             .players
        //             .iter()
        //             .map(|p| &p.kind)
        //             .filter(|p| matches!(p, PlayerKind::User(_)))
        //             .cycle()
        //             .skip_while(|p| **p != d.data.czar)
        //             .skip(1)
        //             .next()
        //             .unwrap();

        //         ui.delete_replies().await?;

        //         if *czar != d.data.czar {
        //             d.data.czar = czar.clone();

        //             let write = write::Write {
        //                 data: std::mem::replace(&mut d.data, DATA_DUMMY),
        //             };
        //             ui.update(i, write.render()).await?;

        //             *self = CAH::Write(write);
        //         } else {
        //             d.choice.shuffle.shuffle(&mut rand::thread_rng());
        //             ui.update(i, d.render()).await?;
        //         }

        //         Flow::Continue
        //     }
        // }
    }
}

#[async_trait]
impl Game for CAH {
    const NAME: &'static str = "Crappy Ableist Humor";
    const COLOR: u32 = 0x000000;

    fn new(user: User) -> Self {
        CAH::Setup(Setup {
            packs: Packs(vec![Arc::new((
                "CAH Base".into(),
                serde_json::from_str(read_to_string("cards/base.json").unwrap().as_str()).unwrap(),
            ))]),
            selected_packs: vec![0],
            bots: 0,
            cards: 10,
            points: 8,
            users: vec![user.id],
        })
    }

    fn lobby_msg_reply(&mut self) -> Flow<GameMessage> {
        Flow::Return(match self {
            CAH::Setup(s) => s.render(Event::none())?.0,
            _ => unreachable!(),
        })
    }
}
