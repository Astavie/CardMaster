use std::collections::HashMap;

use async_trait::async_trait;
use monostate::MustBeU64;

use crate::game::{Flow, Game, GameMessage, GameUI, Logic, Setup, SetupOption};

use discord::{
    interaction::{Interaction, MessageComponent},
    message::{ActionRow, ActionRowComponent, Button, ButtonStyle, Field},
    resource::Snowflake,
    user::User,
};

struct Player {}

pub struct CAH {
    setup: Setup,
    players: HashMap<Snowflake<User>, Player>,
}

#[async_trait]
impl Logic for CAH {
    type Return = ();
    async fn logic(&mut self, ui: &mut GameUI, i: Interaction<MessageComponent>) -> Flow<()> {
        self.setup.update(&i)?;
        ui.update(i.token, self.render_setup()).await.unwrap();

        Flow::Continue
    }
}

impl CAH {
    fn render_setup(&self) -> GameMessage {
        let mut setup = self.setup.render();

        let components = vec![
            ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Success,
                custom_id: "Ej".into(),
                label: Some("Join".into()),
                disabled: false,
            }),
            ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Danger,
                custom_id: "El".into(),
                label: Some("Leave".into()),
                disabled: false,
            }),
            ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Primary,
                custom_id: "Ed".into(),
                label: Some("Done".into()),
                disabled: false,
            }),
        ];

        setup.push(ActionRow {
            typ: MustBeU64::<1>,
            components,
        });

        let mut players = self
            .players
            .keys()
            .map(|id| format!("<@{}>", id))
            .collect::<Vec<_>>()
            .join("\n");
        if players.is_empty() {
            players = "*None.*".into();
        }

        let players = Field::new("Players", players);

        Self::message(vec![players], setup)
    }
}

#[async_trait]
impl Game for CAH {
    const NAME: &'static str = "Crappy Ableist Humor";
    const COLOR: u32 = 0x000000;

    fn new(user: User) -> Self {
        let mut players = HashMap::new();
        players.insert(user.id, Player {});
        CAH {
            players,
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
                ],
            },
        }
    }

    fn lobby_msg_reply(&self) -> GameMessage {
        self.render_setup()
    }
}
