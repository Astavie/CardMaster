use std::fmt::Write;
use std::fmt::{self, Formatter};
use std::marker::ConstParamTy;
use std::ops::Index;
use std::sync::Arc;
use std::{fmt::Display, fs::read_to_string};
use std::{matches, mem};

use async_trait::async_trait;
use discord::message::Field;
use discord::{resource::Snowflake, user::User};
use discord::{DiscordFormatter, DisplayDiscord};
use rand::seq::IteratorRandom;
use rand::{thread_rng, Rng};
use serde::Deserialize;

use crate::enum_str;
use crate::game::widget::Event;
use crate::game::ActionResponse;
use crate::game::{Game, GameMessage};

use self::setup::Setup;

mod read;
mod setup;
mod write;

#[derive(Deserialize)]
#[serde(untagged)]
pub enum CardData {
    Raw(String),
    Full { text: String, pick: usize },
}

impl CardData {
    fn blanks_black(&self) -> usize {
        match *self {
            CardData::Raw(ref s) => s.chars().filter(|&c| c == '_').count().max(1),
            CardData::Full { pick, .. } => pick,
        }
    }
    fn blanks_white(&self) -> usize {
        match *self {
            CardData::Raw(ref s) => s.chars().filter(|&c| c == '_').count(),
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
    player: PlayerKind,
}

impl<const C: CardType> Card<C> {
    pub fn text(self, packs: &Packs) -> &str {
        match &packs[self] {
            CardData::Raw(text) => text,
            CardData::Full { text, .. } => text,
        }
    }
    pub fn is_filled(
        self,
        packs: &Packs,
        white: impl Iterator<Item = Option<Card<{ CardType::White }>>>,
    ) -> bool {
        let mut blanks = match C {
            CardType::White => packs[self].blanks_white(),
            CardType::Black => packs[self].blanks_black(),
        };
        let mut cards = 0;

        for card in white {
            match card {
                Some(card) => {
                    // NOTE: this already accounts for recursiveness
                    blanks += packs[card].blanks_white();
                    cards += 1;
                }
                None => return false,
            }
        }

        cards == blanks
    }
    pub fn fill(
        self,
        packs: &Packs,
        white: &mut impl Iterator<Item = Option<Card<{ CardType::White }>>>,
    ) -> String {
        let mut buf = String::new();
        let mut fmt = DiscordFormatter::new(&mut buf);
        self.fmt(packs, white, &mut fmt).unwrap();
        buf
    }
    pub fn fmt(
        self,
        packs: &Packs,
        white: &mut impl Iterator<Item = Option<Card<{ CardType::White }>>>,
        fmt: &mut DiscordFormatter<'_>,
    ) -> fmt::Result {
        let mut prompt = self.text(packs);

        match C {
            CardType::White => {
                prompt = prompt.trim_end_matches('.');
                fmt.start_code()?;
            }
            CardType::Black => {}
        }

        let inner_start = |fmt: &mut DiscordFormatter| match C {
            CardType::White => fmt.end_code(),
            CardType::Black => write!(fmt, " "),
        };

        let inner_end = |fmt: &mut DiscordFormatter| match C {
            CardType::White => fmt.start_code(),
            CardType::Black => write!(fmt, " "),
        };

        while let Some(pos) = prompt.find(['_', '{']) {
            // trim spaces around blank
            let before = prompt[..pos].trim_end();

            let underscore = prompt.chars().nth(pos).unwrap() == '_';
            let size = if underscore { 1 } else { 2 };
            let after = prompt[pos + size..].trim_start();

            write!(fmt, "{}", before)?;

            if underscore {
                match white.next() {
                    Some(Some(c)) => {
                        inner_start(fmt)?;
                        c.fmt(packs, white, fmt)?;
                        inner_end(fmt)?;
                    }
                    _ => {
                        write!(fmt, "{}", &prompt[pos..pos + size])?;
                    }
                }
            } else {
                inner_start(fmt)?;
                DisplayDiscord::fmt(&self.player, fmt)?;
                inner_end(fmt)?;
            }

            prompt = after;
        }

        write!(fmt, "{}", prompt)?;

        match C {
            CardType::White => {
                fmt.end_code()?;
            }
            CardType::Black => {
                for _ in 0..packs[self].extra_blanks() {
                    if let Some(Some(c)) = white.next() {
                        write!(fmt, " ")?;
                        c.fmt(packs, white, fmt)?;
                    }
                }
            }
        }

        Ok(())
    }
}

pub type Pack = Arc<(String, PackData)>;
pub struct Packs(Vec<Pack>);

impl<const C: CardType> Index<Card<C>> for Packs {
    type Output = CardData;

    fn index(&self, index: Card<C>) -> &Self::Output {
        match C {
            CardType::White => &self.0[index.pack as usize].1.white[index.card as usize],
            CardType::Black => &self.0[index.pack as usize].1.black[index.card as usize],
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum PlayerKind {
    User(Snowflake<User>),
    Rando(usize),
}

impl Display for PlayerKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PlayerKind::User(u) => u.fmt(f),
            PlayerKind::Rando(i) => write!(f, "``Rando Cardrissian #{}``", i + 1),
        }
    }
}

impl DisplayDiscord for PlayerKind {
    fn fmt(&self, f: &mut discord::DiscordFormatter<'_>) -> fmt::Result {
        match self {
            PlayerKind::User(u) => {
                f.unescaped().write_str("<@")?;
                write!(f, "{}", u.as_int())?;
                f.unescaped().write_str(">")?;
                Ok(())
            }
            PlayerKind::Rando(i) => {
                f.start_code()?;
                write!(f, "Rando Cardrissian #{}", i + 1)?;
                f.end_code()?;
                Ok(())
            }
        }
    }
}

pub struct Player {
    pub kind: PlayerKind,
    pub points: i32,
    pub hand: Vec<Card<{ CardType::White }>>,
    pub selected: Vec<Option<usize>>,
}

impl Player {
    pub fn new(kind: PlayerKind) -> Self {
        Self {
            kind,
            points: 0,
            hand: Vec::new(),
            selected: Vec::new(),
        }
    }
    pub fn selected(&self) -> impl Iterator<Item = Option<Card<{ CardType::White }>>> + '_ {
        self.selected
            .iter()
            .copied()
            .map(|i| i.map(|i| self.hand[i]))
    }
}

pub fn draw(
    players: &mut [Player],
    num: usize,
    packs: &mut Packs,
    max: usize,
    prompt: Card<{ CardType::Black }>,
) -> bool {
    let player = &mut players[num];

    // remove selected cards
    let mut selected = std::mem::replace(&mut player.selected, Vec::new());
    selected.sort_unstable_by(|a, b| b.cmp(a));
    for index in selected {
        if let Some(index) = index {
            player.hand.remove(index);
        }
    }
    // draw new cards
    for _ in 0..max - player.hand.len() {
        let draw_white = packs.draw_white(players);
        let player = &mut players[num];
        player.hand.push(match draw_white {
            Some(c) => c,
            None => return false,
        });
    }
    // if rando, give answer immediately
    let player = &mut players[num];
    if matches!(player.kind, PlayerKind::Rando(_)) {
        fn choose(raw: &mut Vec<usize>) -> Option<usize> {
            let i = (0..raw.len()).choose(&mut thread_rng())?;
            Some(raw.swap_remove(i))
        }

        let mut indices: Vec<_> = (0..max).collect();
        while !prompt.is_filled(packs, player.selected()) {
            player.selected.push(Some(match choose(&mut indices) {
                Some(i) => i,
                None => return false,
            }))
        }
    }
    true
}

impl Packs {
    pub fn draw_black(&mut self, players: &[Player]) -> Option<Card<{ CardType::Black }>> {
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
            .rev()
            .find(|&(_, t)| random >= t)
            .unwrap();
        let card = random - start_index;

        let player = players[rng.gen_range(0..players.len())].kind;
        Some(Card {
            pack: pack as u32,
            card: card as u32,
            player,
        })
    }
    pub fn draw_white(&mut self, players: &[Player]) -> Option<Card<{ CardType::White }>> {
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
            .rev()
            .find(|&(_, t)| random >= t)
            .unwrap();
        let card = random - start_index;

        let player = players[rng.gen_range(0..players.len())].kind;
        Some(Card {
            pack: pack as u32,
            card: card as u32,
            player,
        })
    }
}

pub enum CAH {
    Setup(Setup),
    Write(Ingame),
    Read(Ingame),
}

enum_str!(Action: Start, ShowHand, ChangeHand, Continue, Done);
enum_str!(Panel: Main, Hand);

pub struct Ingame {
    pub packs: Packs,
    pub cards: usize,
    pub points: i32,
    pub players: Vec<Player>,

    pub prompt: Card<{ CardType::Black }>,
    pub czar: PlayerKind,
}

#[async_trait]
impl Game for CAH {
    type Action = Action;
    type Panel = Panel;

    const NAME: &'static str = "Crappy Ableist Humor";
    const COLOR: u32 = 0x000000;

    fn create_panel(
        &mut self,
        msg: &mut GameMessage,
        event: &Event,
        panel: Panel,
        user: Snowflake<User>,
    ) -> Option<Action> {
        match self {
            CAH::Setup(s) => s.create(msg, event),
            CAH::Write(i) => i.create_write(msg, event, panel, user),
            CAH::Read(i) => i.create_read(msg, event),
        }
    }

    fn on_action(&mut self, action: Action, _panel: Panel, _user: &User) -> ActionResponse<Panel> {
        if action == Action::Done {
            return ActionResponse::Exit;
        }

        match self {
            CAH::Setup(s) => {
                if action != Action::Start {
                    return ActionResponse::None;
                }

                let players: Vec<_> = s.players().collect();
                let mut packs = Packs(
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
                    return ActionResponse::Error(GameMessage::new(
                        vec![Field::new("Error", "not enough players")],
                        vec![],
                    ));
                }

                let czar = match players.iter().find(|p| matches!(p, PlayerKind::User(_))) {
                    Some(p) => p.clone(),
                    None => {
                        return ActionResponse::Error(GameMessage::new(
                            vec![Field::new(
                                "Error",
                                "I know you want to witness the AI uprising, but you can't play a game with only Rando Cardrissian"
                            )],
                            vec![],
                        ));
                    }
                };

                let mut players = players.into_iter().map(Player::new).collect::<Vec<_>>();

                let prompt = match packs.draw_black(&players) {
                    Some(c) => c,
                    None => {
                        return ActionResponse::Error(GameMessage::new(
                            vec![Field::new(
                                "Error",
                                "selected packs do not have any black cards",
                            )],
                            vec![],
                        ));
                    }
                };

                for num in 0..players.len() {
                    if !draw(&mut players, num, &mut packs, s.cards as usize, prompt) {
                        return ActionResponse::Error(GameMessage::new(
                            vec![Field::new(
                                "Error",
                                "selected packs do not have enough white cards to start",
                            )],
                            vec![],
                        ));
                    }
                }

                // start game!
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
                    *self = CAH::Read(ingame);
                } else {
                    *self = CAH::Write(ingame);
                }

                ActionResponse::NextMain
            }
            CAH::Write(i) => match action {
                Action::ShowHand => ActionResponse::Reply(Panel::Hand),
                Action::ChangeHand => {
                    if i.players
                        .iter()
                        .all(|p| i.czar == p.kind || i.prompt.is_filled(&i.packs, p.selected()))
                    {
                        *self = CAH::Read(unsafe {
                            mem::replace(
                                i,
                                Ingame {
                                    packs: Packs(Vec::new()),
                                    cards: 0,
                                    points: 0,
                                    players: Vec::new(),
                                    prompt: mem::zeroed(),
                                    czar: mem::zeroed(),
                                },
                            )
                        });
                        ActionResponse::NextMain
                    } else {
                        ActionResponse::EditMain
                    }
                }
                _ => ActionResponse::None,
            },
            CAH::Read(i) => {
                if action != Action::Continue {
                    return ActionResponse::None;
                }

                // new prompt
                i.prompt = match i.packs.draw_black(&i.players) {
                    Some(c) => c,
                    None => todo!("no black cards"),
                };

                // draw cards
                for num in 0..i.players.len() {
                    if !draw(&mut i.players, num, &mut i.packs, i.cards, i.prompt) {
                        todo!("no white cards");
                    }
                }

                // new czar
                let czar = i
                    .players
                    .iter()
                    .map(|p| &p.kind)
                    .filter(|p| matches!(p, PlayerKind::User(_)))
                    .cycle()
                    .skip_while(|p| **p != i.czar)
                    .skip(1)
                    .next()
                    .unwrap();

                if *czar != i.czar {
                    i.czar = czar.clone();
                    *self = CAH::Write(unsafe {
                        mem::replace(
                            i,
                            Ingame {
                                packs: Packs(Vec::new()),
                                cards: 0,
                                points: 0,
                                players: Vec::new(),
                                prompt: mem::zeroed(),
                                czar: mem::zeroed(),
                            },
                        )
                    });
                }

                ActionResponse::NextMain
            }
        }
    }

    fn new(user: User) -> Self {
        CAH::Setup(Setup {
            packs: Packs(vec![
                Arc::new((
                    "CAH Base".into(),
                    serde_json::from_str(read_to_string("cards/base.json").unwrap().as_str())
                        .unwrap(),
                )),
                Arc::new((
                    "EPPgroep.".into(),
                    serde_json::from_str(read_to_string("cards/eppgroep.json").unwrap().as_str())
                        .unwrap(),
                )),
                Arc::new((
                    "EPPgroep.".into(),
                    serde_json::from_str(read_to_string("cards/eppgroep.json").unwrap().as_str())
                        .unwrap(),
                )),
                Arc::new((
                    "Modifiers".into(),
                    serde_json::from_str(read_to_string("cards/modifiers.json").unwrap().as_str())
                        .unwrap(),
                )),
                Arc::new((
                    "Modifiers".into(),
                    serde_json::from_str(read_to_string("cards/modifiers.json").unwrap().as_str())
                        .unwrap(),
                )),
                Arc::new((
                    "Modifiers".into(),
                    serde_json::from_str(read_to_string("cards/modifiers.json").unwrap().as_str())
                        .unwrap(),
                )),
                Arc::new((
                    "Modifiers".into(),
                    serde_json::from_str(read_to_string("cards/modifiers.json").unwrap().as_str())
                        .unwrap(),
                )),
            ]),
            selected_packs: vec![0],
            bots: 0,
            cards: 10,
            points: 8,
            users: vec![user.id],
        })
    }
}
