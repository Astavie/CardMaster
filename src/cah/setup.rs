use crate::game::{widget::Event, GameMessage};

use discord::{
    message::{ButtonStyle, Field},
    resource::Snowflake,
    user::User,
};

use super::{Action, Packs, PlayerKind};

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
    pub fn create(&mut self, msg: &mut GameMessage, event: &Event) -> Option<Action> {
        // pack selection
        msg.create_select(
            event,
            "Packs".into(),
            self.packs.0.iter().map(|p| p.0.clone()),
            &mut self.selected_packs,
        );

        // bots
        msg.create_number(event, "Bots".into(), &mut self.bots, 0, i32::MAX);

        // cards
        msg.create_number(event, "Cards".into(), &mut self.cards, 5, 25);

        // points
        msg.create_number(event, "Points".into(), &mut self.points, 1, i32::MAX);

        // players
        msg.create_join(event, &mut self.users);

        let mut players_str = self
            .players()
            .map(|kind| kind.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        if players_str.is_empty() {
            players_str = "*None.*".into();
        }

        msg.fields.push(Field::new("Players", players_str));

        // start button
        msg.append_action(Action::Start, ButtonStyle::Primary, "Start".into());

        None
    }
}
