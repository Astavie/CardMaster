use async_trait::async_trait;
use discord::{
    escape_string,
    interaction::{Interaction, InteractionResource, MessageComponent},
    message::{ActionRow, ActionRowComponent, Button, ButtonStyle, Field},
    resource::Snowflake,
    user::User,
};
use monostate::MustBeU64;

use crate::game::{Flow, GameMessage, GameUI, Logic, Menu};

use super::{Data, PlayerKind};

pub struct Write {
    pub data: Data,
}

impl Write {
    pub fn render_hand(&self, user: Snowflake<User>) -> Option<GameMessage> {
        let player = self
            .data
            .players
            .iter()
            .find(|p| p.kind == PlayerKind::User(user))?;

        let hand = Field::new(
            "Hand",
            player
                .hand
                .iter()
                .copied()
                .enumerate()
                .map(|(i, c)| format!("{}. ``{}``", i + 1, self.data.options.packs[c]))
                .collect::<Vec<_>>()
                .join("\n"),
        );

        if self.data.czar == PlayerKind::User(user) {
            Some(GameMessage::new(vec![hand], Vec::new()))
        } else {
            Some(GameMessage::new(
                vec![
                    Field::new(
                        "Answer",
                        self.data
                            .prompt
                            .fill(&self.data.options.packs, player.selected()),
                    ),
                    hand,
                ],
                player.selected.render(),
            ))
        }
    }
    pub fn render(&self) -> GameMessage {
        let points = self
            .data
            .players
            .iter()
            .map(|p| {
                format!(
                    "{} `{:2}` {}",
                    if p.kind == self.data.czar {
                        "👑"
                    } else if p.selected.disable_unselected {
                        "✅"
                    } else {
                        "💭"
                    },
                    p.points,
                    p.kind,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        GameMessage::new(
            vec![
                Field::new("Players", points),
                Field::new(
                    "Prompt",
                    escape_string(
                        &self.data.options.packs[self.data.prompt]
                            .to_string()
                            .replace("\\n", "\n "),
                    ),
                ),
            ],
            vec![ActionRow {
                typ: MustBeU64::<1>,
                components: vec![ActionRowComponent::Button(Button::Action {
                    style: ButtonStyle::Primary,
                    custom_id: "hand".into(),
                    label: Some("Show Hand".into()),
                    disabled: false,
                })],
            }],
        )
    }
}

#[async_trait]
impl Logic for Write {
    type Return = ();
    async fn logic(
        &mut self,
        ui: &mut GameUI,
        i: Interaction<MessageComponent>,
    ) -> Flow<Self::Return> {
        // show hand
        if i.data.custom_id == "hand" {
            let reply = match self.render_hand(i.user.id) {
                Some(m) => m,
                None => {
                    // TODO: error
                    i.forget();
                    return Flow::Continue;
                }
            };
            ui.reply(i, reply).await.unwrap();
            return Flow::Continue;
        }

        // button grid
        let user = i.user.id;
        let player = self
            .data
            .players
            .iter_mut()
            .find(|p| p.kind == PlayerKind::User(user))?;

        let index = player.selected.update(&i)?;
        match player
            .selected
            .selected
            .iter_mut()
            .find(|o| **o == Some(index))
        {
            Some(o) => _ = o.take(),

            // TODO: remove internal recursive cards
            None => match player.selected.selected.iter_mut().find(|o| o.is_none()) {
                Some(o) => _ = o.insert(index),
                None => player.selected.selected.push(Some(index)),
            },
        }

        let filled = self
            .data
            .prompt
            .is_filled(&self.data.options.packs, player.selected());

        player.selected.disable_unselected = filled;

        // check if every player is done
        if filled
            && self
                .data
                .players
                .iter()
                .all(|p| p.kind == self.data.czar || p.selected.disable_unselected)
        {
            i.forget();
            Flow::Return(())
        } else {
            // rerender
            ui.update(i, self.render_hand(user)?).await.unwrap();
            ui.edit(self.render()).await.unwrap();
            Flow::Continue
        }
    }
}
