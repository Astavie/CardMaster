use async_trait::async_trait;

use crate::game::{self, Flow, GameMessage, GameUI, Logic, SetupOption};

use discord::{
    interaction::{Interaction, MessageComponent},
    message::{ActionRowComponent, Button, ButtonStyle, Field},
    user::User,
};

use super::{Pack, PlayerKind};

pub struct Setup {
    packs: Vec<Pack>,
    options: game::Setup,
}

pub struct Options {
    pub packs: Vec<Pack>,
    pub cards: usize,
    pub points: i32,
}

#[async_trait]
impl Logic for Setup {
    type Return = (Options, Vec<PlayerKind>);
    async fn logic(
        &mut self,
        ui: &mut GameUI,
        i: Interaction<MessageComponent>,
    ) -> Flow<Self::Return> {
        if i.data.custom_id.as_str() == "Es" {
            // Start!
            let packs_list = self.options.options[0].1.as_enabled();
            let packs = self
                .packs
                .iter()
                .zip(packs_list)
                .filter(|(_, &(_, b))| b)
                .map(|(p, _)| p)
                .cloned()
                .collect();

            let points = self.options.options[2].1.as_number();
            let cards = self.options.options[3].1.as_number() as usize;

            Flow::Return((
                Options {
                    packs,
                    points,
                    cards,
                },
                self.players().collect(),
            ))
        } else {
            // Setup
            self.options.update(&i)?;
            ui.update(i.token, self.render()).await.unwrap();
            Flow::Continue
        }
    }
}

impl Setup {
    fn players(&self) -> impl Iterator<Item = PlayerKind> + '_ {
        let bot_count = self.options.options[1].1.as_number() as usize;
        let bots = (0..bot_count).map(|i| PlayerKind::Rando(i));

        let users = self.options.options[4]
            .1
            .as_players()
            .iter()
            .map(|&u| PlayerKind::User(u));

        Iterator::chain(bots, users)
    }
    pub fn new(user: User, packs: Vec<Pack>) -> Self {
        Setup {
            options: game::Setup {
                options: vec![
                    (
                        "Packs".into(),
                        SetupOption::MultiSelect(
                            packs
                                .iter()
                                .enumerate()
                                .map(|(i, pack)| (pack.0.clone(), i == 0))
                                .collect(),
                        ),
                    ),
                    (
                        "Rando Cardrissian".into(),
                        SetupOption::Number(0, i32::MAX, 1),
                    ),
                    ("Max points".into(), SetupOption::Number(1, i32::MAX, 8)),
                    ("Hand cards".into(), SetupOption::Number(5, 20, 10)),
                    ("Players".into(), SetupOption::Players(vec![user.id])),
                ],
            },
            packs,
        }
    }
    pub fn render(&self) -> GameMessage {
        let mut rows = self.options.render();

        // done button
        rows[4]
            .components
            .push(ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Primary,
                custom_id: "Es".into(),
                label: Some("Start".into()),
                disabled: false,
            }));

        // player list
        let mut players_str = self
            .players()
            .map(|kind| kind.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        if players_str.is_empty() {
            players_str = "*None.*".into();
        }

        GameMessage::new(vec![Field::new("Players", players_str)], rows)
    }
}
