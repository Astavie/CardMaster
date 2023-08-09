use discord::{
    escape_string,
    message::{ButtonStyle, Field},
    resource::Snowflake,
    user::User,
};

use crate::game::{widget::Event, GameMessage};

use super::{Action, Ingame, Panel, PlayerKind};

impl Ingame {
    pub fn create_write(
        &mut self,
        msg: &mut GameMessage,
        event: &Event,
        panel: Panel,
        user: Snowflake<User>,
    ) -> Option<Action> {
        match panel {
            Panel::Main => {
                msg.fields.push(Field::new(
                    "Players",
                    self.players
                        .iter()
                        .map(|p| {
                            format!(
                                "{} `{:2}` {}",
                                if p.kind == self.czar {
                                    "👑"
                                } else if self.prompt.is_filled(&self.packs, p.selected()) {
                                    "✅"
                                } else {
                                    "💭"
                                },
                                p.points,
                                p.kind,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ));

                msg.fields.push(Field::new(
                    "Prompt",
                    escape_string(&self.packs[self.prompt].to_string().replace("\\n", "\n ")),
                ));

                msg.append_action(Action::ShowHand, ButtonStyle::Primary, "Show Hand".into());
                None
            }
            Panel::Hand => {
                let player = self
                    .players
                    .iter_mut()
                    .find(|p| p.kind == PlayerKind::User(user))?;

                let mut changed = false;
                if self.czar != PlayerKind::User(user) {
                    changed = msg.create_select_grid(
                        event,
                        self.cards,
                        &mut player.selected,
                        |selected| {
                            self.prompt.is_filled(
                                &self.packs,
                                selected.iter().map(|o| o.map(|p| player.hand[p])),
                            )
                        },
                    );

                    msg.fields.push(Field::new(
                        "Answer",
                        self.prompt.fill(&self.packs, &mut player.selected()),
                    ));
                }

                msg.fields.push(Field::new(
                    "Hand",
                    player
                        .hand
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(i, c)| format!("{}. ``{}``", i + 1, self.packs[c]))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ));

                if changed {
                    Some(Action::ChangeHand)
                } else {
                    None
                }
            }
        }
    }
}
