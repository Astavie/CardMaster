use std::{
    collections::HashMap,
    convert::Infallible,
    ops::{ControlFlow, FromResidual, Try},
};

use async_trait::async_trait;
use futures_util::future::join_all;

use discord::{
    channel::ChannelResource,
    interaction::{
        ApplicationCommand, ComponentInteractionResource, CreateReply, CreateUpdate, Interaction,
        InteractionResource, InteractionResponseIdentifier, InteractionToken, MessageComponent,
        ReplyFlag, Webhook,
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

use self::ui::{Event, Widget};

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

    replies: HashMap<Snowflake<Message>, InteractionResponseIdentifier>,
}

pub struct GameMessage {
    pub fields: Vec<Field>,
    pub components: Vec<ActionRow>,
}

impl GameMessage {
    pub fn new(fields: Vec<Field>, components: Vec<ActionRow>) -> Self {
        Self { fields, components }
    }
    pub fn create<T>(&mut self, event: &Event, widget: impl Widget<Result = T>) -> Flow<Option<T>> {
        widget.create(self, event)
    }
}

impl From<Message> for GameMessage {
    fn from(value: Message) -> Self {
        GameMessage {
            fields: value.embeds.into_iter().next().unwrap().fields,
            components: value.components,
        }
    }
}

impl Widget for GameMessage {
    type Result = ();

    fn create(self, msg: &mut GameMessage, _event: &Event) -> Flow<Option<()>> {
        msg.fields.extend(self.fields);
        msg.components.extend(self.components);
        Flow::Return(Some(()))
    }
}

impl GameUI {
    pub fn base_message_id(&self) -> Snowflake<Message> {
        self.msg_id
    }
    pub async fn edit<T>(
        &self,
        id: Snowflake<Message>,
        widget: impl Widget<Result = T>,
    ) -> Flow<T> {
        let (msg, res) = widget.render(Event::none())?;

        if id == self.msg_id {
            // sign if we are updating the base message
            self.msg
                .as_ref()
                .unwrap()
                .patch(
                    &Webhook,
                    PatchMessage::default()
                        .embeds(vec![Embed::default()
                            .author(Author::new(self.name))
                            .color(self.color)
                            .fields(msg.fields)])
                        .components(msg.components),
                )
                .await
                .unwrap();
        } else {
            self.replies[&id]
                .patch(
                    &Webhook,
                    PatchMessage::default()
                        .embeds(vec![Embed::default().fields(msg.fields)])
                        .components(msg.components),
                )
                .await
                .unwrap();
        }

        Flow::Return(res?)
    }
    pub async fn reply<T>(
        &mut self,
        i: Interaction<MessageComponent>,
        widget: impl Widget<Result = T>,
    ) -> Flow<T> {
        let (msg, res) = widget.render(Event::component(&i))?;

        // we do not sign replies

        let response = i
            .reply(
                &Webhook,
                CreateReply::default()
                    .embeds(vec![Embed::default().fields(msg.fields)])
                    .components(msg.components)
                    .flags(ReplyFlag::Ephemeral.into()),
            )
            .await
            .unwrap();

        let id = response.get(&Webhook).await.unwrap().id.snowflake();
        self.replies.insert(id, response);

        Flow::Return(res?)
    }
    pub async fn update<T>(
        &mut self,
        i: Interaction<MessageComponent>,
        widget: impl Widget<Result = T>,
    ) -> Flow<T> {
        let (msg, res) = widget.render(Event::component(&i))?;

        if i.data.message.id.snowflake() == self.msg_id {
            // sign if we are updating the base message
            self.msg = Some(
                i.update(
                    &Webhook,
                    CreateUpdate::default()
                        .embeds(vec![Embed::default()
                            .author(Author::new(self.name))
                            .color(self.color)
                            .fields(msg.fields)])
                        .components(msg.components),
                )
                .await
                .unwrap(),
            );
        } else {
            i.update(
                &Webhook,
                CreateUpdate::default()
                    .embeds(vec![Embed::default().fields(msg.fields)])
                    .components(msg.components),
            )
            .await
            .unwrap();
        }

        Flow::Return(res?)
    }

    pub async fn delete(&mut self, i: Interaction<MessageComponent>) -> Flow<()> {
        let msg = i.data.message.id.snowflake();
        if let Some(id) = self.replies.remove(&msg) {
            let _ = id.delete(&Webhook).await;
        }
        i.forget();
        Flow::Return(())
    }
    pub async fn delete_replies(&mut self) -> Flow<()> {
        let _ = join_all(self.replies.drain().map(|(_, id)| id.delete(&Webhook))).await;
        Flow::Return(())
    }
}

#[must_use]
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
        let msg = me.lobby_msg_reply();

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
                            .embeds(vec![Embed::default()
                                .author(Author::new(Self::NAME))
                                .color(Self::COLOR)
                                .fields(msg.fields)])
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
                            .embeds(vec![Embed::default()
                                .author(Author::new(Self::NAME))
                                .color(Self::COLOR)
                                .fields(msg.fields)])
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
