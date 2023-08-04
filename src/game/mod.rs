use std::{
    collections::HashMap,
    convert::Infallible,
    ops::{ControlFlow, FromResidual, Try},
};

use async_trait::async_trait;
use futures_util::future::try_join_all;

use discord::{
    channel::ChannelResource,
    interaction::{
        ApplicationCommand, ComponentInteractionResource, CreateReply, CreateUpdate, Interaction,
        InteractionResource, InteractionResponseIdentifier, InteractionResponseResource,
        InteractionToken, MessageComponent, ReplyFlag, Webhook,
    },
    message::{
        ActionRow, Author, CreateMessage, Embed, Field, Message, MessageResource, PatchMessage,
    },
    request::{Discord, Result},
    resource::Snowflake,
    user::User,
};

// TODO: convert setup to a UI system
mod setup;
pub use setup::{Setup, SetupOption};

pub mod ui;

pub const B64_TABLE: [char; 64] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l',
    'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '+', '/',
];

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

        let pos = match self
            .games
            .iter()
            .position(|s| s.ui.msg_id == msg || s.ui.replies.contains_key(&msg))
        {
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

    msg_id: Snowflake<Message>,
    msg: Option<InteractionResponseIdentifier>,

    replies: HashMap<Snowflake<Message>, (Snowflake<User>, InteractionResponseIdentifier)>,
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
    pub async fn edit(&self, message: GameMessage) -> Result<()> {
        let message = message.sign(self);
        self.msg
            .as_ref()
            .unwrap()
            .patch(
                &Webhook,
                PatchMessage::default()
                    .embeds(vec![message.embed])
                    .components(message.components),
            )
            .await?;
        Ok(())
    }
    pub async fn reply(
        &mut self,
        i: Interaction<MessageComponent>,
        message: GameMessage,
    ) -> Result<()> {
        let user = i.user.id;

        // we do not sign replies

        let response = i
            .reply(
                &Webhook,
                CreateReply::default()
                    .embeds(vec![message.embed])
                    .components(message.components)
                    .flags(ReplyFlag::Ephemeral.into()),
            )
            .await?;

        let id = response.get(&Webhook).await?.id.snowflake();

        self.replies.insert(id, (user, response));

        Ok(())
    }
    pub async fn update(
        &mut self,
        i: Interaction<MessageComponent>,
        mut message: GameMessage,
    ) -> Result<()> {
        if i.data.message.id.snowflake() == self.msg_id {
            // sign if we are updating the base message
            message = message.sign(self);
            self.msg = Some(
                i.update(
                    &Webhook,
                    CreateUpdate::default()
                        .embeds(vec![message.embed])
                        .components(message.components),
                )
                .await?,
            );
        } else {
            i.update(
                &Webhook,
                CreateUpdate::default()
                    .embeds(vec![message.embed])
                    .components(message.components),
            )
            .await?;
        }
        Ok(())
    }
    pub async fn delete(&mut self, i: Interaction<MessageComponent>) -> Result<()> {
        let msg = i.data.message.id.snowflake();
        if let Some((_, id)) = self.replies.remove(&msg) {
            id.delete(&Webhook).await?;
        }
        i.forget();
        Ok(())
    }
    pub async fn delete_replies(&mut self) -> Result<()> {
        try_join_all(self.replies.drain().map(|(_, (_, id))| id.delete(&Webhook))).await?;
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

pub trait Menu {
    type Update;
    fn render(&self) -> Vec<ActionRow>;
    fn update(&mut self, it: &Interaction<MessageComponent>) -> Flow<Self::Update>;
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

    async fn start(
        token: InteractionToken<ApplicationCommand>,
        user: User,
        thread: Option<&Discord>,
    ) -> Result<GameTask> {
        let me = Self::new(user);

        // send lobby message
        let mut msg = me.lobby_msg_reply();
        msg.embed = msg.embed.author(Author::new(Self::NAME)).color(Self::COLOR);

        let (id, msg) = match thread {
            Some(discord) => {
                // TODO: close thread on end
                // TODO: give thread better name
                let id = token
                    .reply(
                        &Webhook,
                        CreateReply::default()
                            .content(format!("A new game of ``{}`` is starting!", Self::NAME)),
                    )
                    .await?;
                let channel = id
                    .get(&Webhook)
                    .await?
                    .start_thread(discord, Self::NAME.into())
                    .await?;
                let msg = channel
                    .send_message(
                        discord,
                        CreateMessage::default()
                            .embeds(vec![msg.embed])
                            .components(msg.components),
                    )
                    .await?;
                (None, msg)
            }
            None => {
                let id = token
                    .reply(
                        &Webhook,
                        CreateReply::default()
                            .embeds(vec![msg.embed])
                            .components(msg.components),
                    )
                    .await?;
                let msg = id.get(&Webhook).await?;
                (Some(id), msg)
            }
        };

        // create task
        Ok(GameTask {
            ui: GameUI {
                name: Self::NAME,
                color: Self::COLOR,
                msg: id,
                msg_id: msg.id.snowflake(),
                replies: HashMap::new(),
            },
            game: Box::new(me),
        })
    }
}
