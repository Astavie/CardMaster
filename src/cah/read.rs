use async_trait::async_trait;
use discord::{
    interaction::{Interaction, MessageComponent},
    message::{ActionRow, ActionRowComponent, Button, ButtonStyle, Field},
};
use monostate::MustBeU64;

use crate::game::{ui::ChoiceGrid, Flow, GameMessage, GameUI, Logic, Menu};

use super::{Data, PlayerKind};

pub struct Read {
    pub data: Data,
    pub choice: ChoiceGrid,
}

impl Read {
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
                    } else {
                        "✅"
                    },
                    p.points,
                    p.kind,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let answers = self
            .choice
            .shuffle
            .iter()
            .copied()
            .map(|i| {
                self.data
                    .players
                    .iter()
                    .filter(|p| p.kind != self.data.czar)
                    .nth(i)
                    .unwrap()
            })
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "{}. {}",
                    i + 1,
                    self.data
                        .prompt
                        .fill(&self.data.options.packs, p.selected())
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        GameMessage::new(
            vec![
                Field::new("Players", points),
                Field::new("Answers", answers),
            ],
            self.choice.render(),
        )
    }
}

#[async_trait]
impl Logic for Read {
    type Return = Interaction<MessageComponent>;
    async fn logic(
        &mut self,
        ui: &mut GameUI,
        i: Interaction<MessageComponent>,
    ) -> Flow<Self::Return> {
        if i.data.custom_id == "continue" {
            return Flow::Return(i);
        }

        // select winner
        let index = self.choice.update(&i)?;

        if self.data.czar != PlayerKind::User(i.user.id) {
            ui.reply(
                i,
                GameMessage::new(
                    vec![Field::new("Error", "you are not the Card Czar")],
                    vec![],
                ),
            )
            .await
            .unwrap();
            return Flow::Continue;
        }

        let winner = self
            .data
            .players
            .iter_mut()
            .filter(|p| p.kind != self.data.czar)
            .nth(index)?;

        winner.points += 1;
        let total_points = winner.points;

        let name = winner.kind.to_string();
        let answer = self
            .data
            .prompt
            .fill(&self.data.options.packs, winner.selected());

        let points = self
            .data
            .players
            .iter()
            .map(|p| format!("`{:2}` {}", p.points, p.kind,))
            .collect::<Vec<_>>()
            .join("\n");

        if total_points >= self.data.options.points {
            ui.update(
                i,
                GameMessage::new(
                    vec![
                        Field::new("Players", points),
                        Field::new(
                            "We have a winner!",
                            format!("{} won the game with `{}` points!", name, total_points),
                        ),
                        Field::new("Last words", format!(">>> {}", answer)),
                    ],
                    vec![],
                ),
            )
            .await
            .unwrap();
            Flow::Exit
        } else {
            ui.update(
                i,
                GameMessage::new(
                    vec![
                        Field::new("Players", points),
                        Field::new("Round Winner", format!("{}\n\n>>> {}", name, answer)),
                    ],
                    vec![ActionRow {
                        typ: MustBeU64,
                        components: vec![ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: "continue".into(),
                            label: Some("Continue".into()),
                            disabled: false,
                        })],
                    }],
                ),
            )
            .await
            .unwrap();
            Flow::Continue
        }
    }
}
