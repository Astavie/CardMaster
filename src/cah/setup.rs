use crate::game::{
    widget::{Event, JoinButtons, MultiSelect, NumberSelect, Widget},
    Flow, GameMessage,
};

use discord::{message::Field, resource::Snowflake, user::User};

use super::{Packs, PlayerKind};

pub struct Setup {
    pub packs: Packs,
    pub selected_packs: Vec<usize>,

    pub bots: i32,
    pub cards: i32,
    pub points: i32,
    pub users: Vec<Snowflake<User>>,
}

impl Setup {
    pub fn players(&self) -> impl Iterator<Item = PlayerKind> + '_ {
        let bots = (0..self.bots).map(|i| PlayerKind::Rando(i as usize));
        let users = self.users.iter().map(|&u| PlayerKind::User(u));
        Iterator::chain(users, bots)
    }
}

impl Widget for &mut Setup {
    fn create(self, msg: &mut GameMessage, event: &Event) -> Flow<bool> {
        // pack selection
        msg.create(
            event,
            MultiSelect::new(
                "packs",
                self.packs.0.iter().map(|p| p.0.clone()),
                &mut self.selected_packs,
            )
            .name("Packs".into()),
        )?;

        // bots
        msg.create(
            event,
            NumberSelect::new("bots", &mut self.bots).name("Rando".into()),
        )?;

        // cards
        msg.create(
            event,
            NumberSelect::new("cards", &mut self.cards)
                .min(5)
                .max(25)
                .name("Cards".into()),
        )?;

        // points
        msg.create(
            event,
            NumberSelect::new("cards", &mut self.points)
                .min(1)
                .name("Points".into()),
        )?;

        // players
        msg.create(event, JoinButtons(&mut self.users))?;

        let mut players_str = self
            .players()
            .map(|kind| kind.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        if players_str.is_empty() {
            players_str = "*None.*".into();
        }

        msg.fields.push(Field::new("Players", players_str));

        Flow::Return(false)
    }
}
