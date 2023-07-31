use std::unreachable;

use async_trait::async_trait;

use crate::{
    game::{Flow, Game, GameMessage, GameUI, Logic},
    setup::{Setup, SetupOption},
};

use discord::{
    interaction::{Interaction, MessageComponent},
    message::{ActionRowComponent, Button, ButtonStyle, Field},
    user::User,
};

pub struct CAH {
    setup: Setup,
}

#[async_trait]
impl Logic for CAH {
    type Return = ();
    async fn logic(&mut self, ui: &mut GameUI, i: Interaction<MessageComponent>) -> Flow<()> {
        if i.data.custom_id.as_str() == "Es" {
            // Start!
            // TODO: start
            Flow::Return(())
        } else {
            // Setup
            self.setup.update(&i)?;
            ui.update(i.token, self.render_setup()).await.unwrap();
            Flow::Continue
        }
    }
}

impl CAH {
    fn render_setup(&self) -> GameMessage {
        let mut rows = self.setup.render();

        rows[4]
            .components
            .push(ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Primary,
                custom_id: "Es".into(),
                label: Some("Start".into()),
                disabled: false,
            }));

        let players_list = match self.setup.options[4].1 {
            SetupOption::Players(ref list) => list,
            _ => unreachable!(),
        };

        let mut players_str = players_list
            .iter()
            .map(|&id| format!("<@{}>", id))
            .collect::<Vec<_>>()
            .join("\n");

        if players_str.is_empty() {
            players_str = "*None.*".into();
        }

        Self::message(vec![Field::new("Players", players_str)], rows)
    }
}

#[async_trait]
impl Game for CAH {
    const NAME: &'static str = "Crappy Ableist Humor";
    const COLOR: u32 = 0x000000;

    fn new(user: User) -> Self {
        CAH {
            setup: Setup {
                options: vec![
                    (
                        "Packs".into(),
                        SetupOption::MultiSelect(vec![
                            ("CAH Base".into(), true),
                            ("EPPgroep".into(), false),
                        ]),
                    ),
                    (
                        "Rules".into(),
                        SetupOption::Flags(vec![
                            ("Rando Cardrissian".into(), true),
                            ("Double or nothing".into(), true),
                        ]),
                    ),
                    ("Max points".into(), SetupOption::Number(1, i32::MAX, 8)),
                    ("Hand cards".into(), SetupOption::Number(5, 20, 10)),
                    ("Players".into(), SetupOption::Players(vec![user.id])),
                ],
            },
        }
    }

    fn lobby_msg_reply(&self) -> GameMessage {
        self.render_setup()
    }
}
