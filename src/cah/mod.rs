use std::fmt::{self, Formatter};
use std::marker::ConstParamTy;
use std::sync::Arc;
use std::{fmt::Display, fs::read_to_string};

use async_trait::async_trait;
use discord::{
    interaction::{Interaction, MessageComponent},
    resource::Snowflake,
    user::User,
};
use serde::Deserialize;

use crate::game::{Flow, Game, GameMessage, GameUI, Logic};

mod read;
mod setup;
mod write;

#[derive(Deserialize)]
#[serde(untagged)]
pub enum CardData {
    Raw(String),
    Full { text: String, pick: usize },
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

#[derive(Clone, Copy)]
pub struct Card<const TYPE: CardType> {
    pack: u32,
    card: u32,
}

pub type Pack = Arc<(String, PackData)>;

pub enum PlayerKind {
    User(Snowflake<User>),
    Rando(usize),
}

impl Display for PlayerKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PlayerKind::User(u) => u.fmt(f),
            PlayerKind::Rando(i) => write!(f, "`Rando Cardrissian #{}`", i + 1),
        }
    }
}

pub struct Player {
    pub kind: PlayerKind,
    pub points: i32,
    pub hand: Vec<Card<{ CardType::White }>>,
}

impl Player {
    pub fn new(kind: PlayerKind) -> Self {
        Self {
            kind,
            points: 0,
            hand: Vec::new(),
        }
    }
}

pub struct Data {
    pub options: setup::Options,
    pub players: Vec<Player>,
}

pub enum CAH {
    Setup(setup::Setup),
    Write(write::Write),
    Read(read::Read),
}

#[async_trait]
impl Logic for CAH {
    type Return = ();
    async fn logic(&mut self, ui: &mut GameUI, i: Interaction<MessageComponent>) -> Flow<()> {
        match self {
            CAH::Setup(s) => {
                let (options, players) = s.logic(ui, i).await?;
                let players = players.into_iter().map(Player::new).collect();

                // TODO: check if packs have enough cards
                // TODO: give players cards

                *self = CAH::Write(write::Write {
                    data: Data { options, players },
                });
                Flow::Continue
            }
            CAH::Write(d) => {
                d.logic(ui, i).await?;
                // TODO
                Flow::Continue
            }
            CAH::Read(d) => {
                d.logic(ui, i).await?;
                // TODO
                Flow::Continue
            }
        }
    }
}

#[async_trait]
impl Game for CAH {
    const NAME: &'static str = "Crappy Ableist Humor";
    const COLOR: u32 = 0x000000;

    fn new(user: User) -> Self {
        CAH::Setup(setup::Setup::new(
            user,
            vec![Arc::new((
                "CAH Base".into(),
                serde_json::from_str(read_to_string("cards/base.json").unwrap().as_str()).unwrap(),
            ))],
        ))
    }

    fn lobby_msg_reply(&self) -> GameMessage {
        match self {
            CAH::Setup(s) => s.render(),
            _ => unreachable!(),
        }
    }
}
