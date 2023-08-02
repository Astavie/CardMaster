use std::{
    convert::Infallible,
    ops::{ControlFlow, FromResidual, Try},
};

use async_trait::async_trait;

use discord::{
    interaction::{
        ApplicationCommand, ComponentInteractionResource, Interaction, InteractionResource,
        InteractionResponseIdentifier, InteractionToken, MessageComponent, ReplyFlag, Webhook,
    },
    message::{ActionRow, Author, Embed, Field, Message},
    request::Result,
    resource::{Patchable, Resource, Snowflake},
    user::User,
};

mod setup;
pub use setup::{Setup, SetupOption};

pub struct InteractionDispatcher {
    games: Vec<GameTask>,
}

pub struct GameTask {
    ui: GameUI,
    game: Box<dyn Logic<Return = ()>>,
}

impl InteractionDispatcher {
    pub fn new() -> Self {
        InteractionDispatcher { games: Vec::new() }
    }
    pub async fn dispatch(&mut self, i: Interaction<MessageComponent>) {
        let msg = i.data.message.id.snowflake();

        let pos = match self.games.iter().position(|s| s.ui.msg_id == msg) {
            Some(pos) => pos,
            _ => {
                // give a "no response" error
                i.forget();
                return;
            }
        };

        let task = &mut self.games[pos];
        let result = task.game.logic(&mut task.ui, i).await;

        if result.is_done() {
            self.games.swap_remove(pos);
        }
    }
    pub fn register(&mut self, task: GameTask) {
        self.games.push(task);
    }
}

pub struct GameUI {
    name: &'static str,
    color: u32,

    msg: InteractionResponseIdentifier,
    msg_id: Snowflake<Message>,
}

pub struct GameMessage {
    pub embed: Embed,
    pub components: Vec<ActionRow>,
}

impl GameMessage {
    fn sign(mut self, ui: &GameUI) -> Self {
        self.embed = self.embed.author(Author::new(ui.name)).color(ui.color);
        self
    }
    pub fn new(fields: Vec<Field>, components: Vec<ActionRow>) -> Self {
        Self {
            embed: Embed::default().fields(fields),
            components,
        }
    }
}

impl From<Message> for GameMessage {
    fn from(value: Message) -> Self {
        GameMessage {
            embed: value.embeds.into_iter().next().unwrap(),
            components: value.components,
        }
    }
}

impl GameUI {
    pub async fn push(&mut self, message: GameMessage) -> Result<()> {
        let message = message.sign(self);
        let (id, m) = self
            .msg
            .followup(&Webhook, |m| {
                m.embeds(vec![message.embed]).components(message.components)
            })
            .await?;
        self.msg = id;
        self.msg_id = m.id.snowflake();
        Ok(())
    }
    pub async fn edit(&self, message: GameMessage) -> Result<()> {
        let message = message.sign(self);
        self.msg
            .patch(&Webhook, |m| {
                m.embeds(vec![message.embed]).components(message.components)
            })
            .await?;
        Ok(())
    }
    pub async fn reply(
        &self,
        i: InteractionToken<MessageComponent>,
        message: GameMessage,
    ) -> Result<()> {
        // We do not sign the message here as this will be an ephemeral reply
        i.reply(&Webhook, |m| {
            m.embeds(vec![message.embed])
                .components(message.components)
                .flags(ReplyFlag::Ephemeral.into())
        })
        .await?;
        Ok(())
    }
    pub async fn update(
        &self,
        i: InteractionToken<MessageComponent>,
        message: GameMessage,
    ) -> Result<()> {
        let message = message.sign(self);
        i.update(&Webhook, |m| {
            m.embeds(vec![message.embed]).components(message.components)
        })
        .await?;
        Ok(())
    }
}

pub enum Flow<T> {
    Return(T),
    Continue,
    Exit,
}

impl<T> Flow<T> {
    pub fn is_done(&self) -> bool {
        match self {
            Self::Continue => false,
            _ => true,
        }
    }
}

impl<T> Try for Flow<T> {
    type Output = T;
    type Residual = Flow<Infallible>;

    fn from_output(output: Self::Output) -> Self {
        Self::Return(output)
    }

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            Flow::Return(t) => ControlFlow::Continue(t),
            Flow::Continue => ControlFlow::Break(Flow::Continue),
            Flow::Exit => ControlFlow::Break(Flow::Exit),
        }
    }
}

impl<T> FromResidual for Flow<T> {
    fn from_residual(residual: Flow<Infallible>) -> Self {
        match residual {
            Flow::Continue => Flow::Continue,
            Flow::Exit => Flow::Exit,
        }
    }
}

impl<T> FromResidual<Option<Infallible>> for Flow<T> {
    fn from_residual(residual: Option<Infallible>) -> Self {
        match residual {
            None => Flow::Continue,
        }
    }
}

#[async_trait]
pub trait Logic {
    type Return;
    async fn logic(
        &mut self,
        ui: &mut GameUI,
        i: Interaction<MessageComponent>,
    ) -> Flow<Self::Return>;
}

#[async_trait]
pub trait Game: Logic<Return = ()> + Sized + 'static {
    const NAME: &'static str;
    const COLOR: u32;

    fn new(user: User) -> Self;
    fn lobby_msg_reply(&self) -> GameMessage;

    async fn start(token: InteractionToken<ApplicationCommand>, user: User) -> Result<GameTask> {
        let me = Self::new(user);

        // send lobby message
        let mut msg = me.lobby_msg_reply();
        msg.embed = msg.embed.author(Author::new(Self::NAME)).color(Self::COLOR);

        let id = token
            .reply(&Webhook, |m| {
                m.embeds(vec![msg.embed]).components(msg.components)
            })
            .await?;
        let msg = id.get(&Webhook).await?;

        // create task
        Ok(GameTask {
            ui: GameUI {
                name: Self::NAME,
                color: Self::COLOR,
                msg: id,
                msg_id: msg.id.snowflake(),
            },
            game: Box::new(me),
        })
    }
}
