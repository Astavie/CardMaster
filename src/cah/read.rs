use discord::message::{ActionRow, ActionRowComponent, Button, ButtonStyle, Field};
use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};

use crate::game::{widget::Event, GameMessage, B64_TABLE};

use super::{Action, Ingame, Player, PlayerKind};

impl Ingame {
    pub fn random_indices(&self) -> Vec<&Player> {
        let mut indices: Vec<_> = self
            .players
            .iter()
            .filter(|p| p.kind != self.czar)
            .collect();
        let mut rng: StdRng =
            SeedableRng::seed_from_u64((self.prompt.pack as u64) << 32 | (self.prompt.card as u64));
        indices.shuffle(&mut rng);
        indices
    }
    pub fn create_read(&mut self, msg: &mut GameMessage, event: &Event) -> Option<Action> {
        if let PlayerKind::User(user) = self.czar {
            if let Some(i) = event.matches(|i| {
                if i.user.id != user {
                    None
                } else {
                    let s = i.data.custom_id.strip_prefix('#')?;
                    let c = s.chars().next()?;
                    B64_TABLE
                        .iter()
                        .position(|&p| p == c)
                        .filter(|&i| i < self.players.len() - 1)
                }
            }) {
                return self.create_winner(msg, i);
            }
        }

        msg.fields.push(Field::new(
            "Players",
            self.players
                .iter()
                .map(|p| {
                    format!(
                        "{} `{:2}` {}",
                        if p.kind == self.czar { "👑" } else { "✅" },
                        p.points,
                        p.kind,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ));

        msg.fields.push(Field::new(
            "Answers",
            self.random_indices()
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    format!(
                        "{}. {}",
                        i + 1,
                        self.prompt.fill(&self.packs, &mut p.selected())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ));

        // picker
        let mut iter = 0..self.players.len() - 1;
        loop {
            let mut buttons = Vec::new();
            for _ in 0..5 {
                match iter.next() {
                    Some(i) => {
                        buttons.push(ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: format!("#{}", B64_TABLE[i]),
                            label: Some((i + 1).to_string()),
                            disabled: false,
                        }));
                    }
                    None => {
                        if !buttons.is_empty() {
                            msg.components.push(ActionRow::new(buttons));
                        }
                        return None;
                    }
                }
            }
            msg.components.push(ActionRow::new(buttons));
        }
    }
    fn create_winner(&mut self, msg: &mut GameMessage, i: usize) -> Option<Action> {
        let mut indices: Vec<_> = self
            .players
            .iter_mut()
            .filter(|p| p.kind != self.czar)
            .collect();
        let mut rng: StdRng =
            SeedableRng::seed_from_u64((self.prompt.pack as u64) << 32 | (self.prompt.card as u64));
        indices.shuffle(&mut rng);

        let winner = &mut *indices[i];
        winner.points += 1;
        let total_points = winner.points;

        let name = winner.kind.to_string();
        let answer = self.prompt.fill(&self.packs, &mut winner.selected());

        let points = self
            .players
            .iter()
            .map(|p| format!("`{:2}` {}", p.points, p.kind,))
            .collect::<Vec<_>>()
            .join("\n");

        return if total_points >= self.points {
            msg.fields.extend(vec![
                Field::new("Players", points),
                Field::new(
                    "We have a winner!",
                    format!("{} won the game with `{}` points!", name, total_points),
                ),
                Field::new("Last words", format!(">>> {}", answer)),
            ]);
            Some(Action::Done)
        } else {
            msg.fields.extend(vec![
                Field::new("Players", points),
                Field::new("Round Winner", format!("{}\n\n>>> {}", name, answer)),
            ]);
            msg.append_action(Action::Continue, ButtonStyle::Primary, "Continue".into());
            None
        };
    }
}
