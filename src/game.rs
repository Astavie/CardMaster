use std::{
    collections::HashMap,
    format,
    ops::{ControlFlow, FromResidual, Try},
};

use async_trait::async_trait;
use monostate::MustBeU64;

use discord::{
    channel::{Channel, ChannelResource},
    interaction::{
        ApplicationCommand, Interaction, InteractionResource, InteractionToken, MessageComponent,
        ReplyFlag,
    },
    message::{
        ActionRow, ActionRowComponent, Author, ButtonStyle, Embed, Field, Message,
        MessageIdentifier, MessageResource,
    },
    request::{Discord, Result},
    resource::{Patchable, Resource, Snowflake},
};

pub struct InteractionDispatcher {
    lobbies: HashMap<Snowflake<Message>, Snowflake<Channel>>,
    channels: HashMap<Snowflake<Channel>, GameTask>,
}

pub struct GameTask {
    ui: GameUI,
    game: Box<dyn Logic<()> + Send>,
}

impl InteractionDispatcher {
    pub fn new() -> Self {
        InteractionDispatcher {
            lobbies: HashMap::new(),
            channels: HashMap::new(),
        }
    }
    pub async fn dispatch(&mut self, client: &Discord, i: Interaction<MessageComponent>) {
        let task = if let Some(s) = self.lobbies.get(&i.data.message.id.snowflake()) {
            self.channels.get_mut(s).unwrap()
        } else if let Some(s) = self.channels.get_mut(&i.channel_id) {
            s
        } else {
            return;
        };

        let result = task.game.logic(client, &mut task.ui, i).await;

        if result.is_done() {
            let channel = task.ui.channel;
            let lobby = task.ui.lobby_msg.snowflake();
            self.channels.remove(&channel);
            self.lobbies.remove(&lobby);
        }
    }
    pub fn register(&mut self, task: GameTask) {
        self.lobbies
            .insert(task.ui.lobby_msg.snowflake(), task.ui.channel);
        self.channels.insert(task.ui.channel, task);
    }
}

pub struct GameUI {
    pub channel: Snowflake<Channel>,
    pub lobby_msg: MessageIdentifier,
    pub state_msg: Option<MessageIdentifier>,
}

pub struct GameMessage {
    pub embed: Embed,
    pub components: Vec<ActionRow>,
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
    pub async fn edit_lobby(&self, client: &Discord, message: GameMessage) -> Result<()> {
        self.lobby_msg
            .patch(client, |m| {
                m.embeds(vec![message.embed]).components(message.components)
            })
            .await?;
        Ok(())
    }
    pub async fn edit_state(&mut self, client: &Discord, message: GameMessage) -> Result<()> {
        match self.state_msg {
            Some(msg) => {
                msg.patch(client, |m| {
                    m.embeds(vec![message.embed]).components(message.components)
                })
                .await?;
            }
            None => {
                self.push_state(client, message).await?;
            }
        }
        Ok(())
    }
    pub async fn push_state(&mut self, client: &Discord, message: GameMessage) -> Result<()> {
        let msg = self
            .channel
            .send_message(client, |m| {
                m.embeds(vec![message.embed]).components(message.components)
            })
            .await?;
        self.state_msg = Some(msg.id);
        Ok(())
    }
    pub async fn reply(
        &self,
        client: &Discord,
        i: InteractionToken<MessageComponent>,
        message: GameMessage,
    ) -> Result<()> {
        i.reply(client, |m| {
            m.embeds(vec![message.embed])
                .components(message.components)
                .flags(ReplyFlag::Ephemeral.into())
        })
        .await?;
        Ok(())
    }
    pub async fn update(
        &self,
        client: &Discord,
        i: InteractionToken<MessageComponent>,
        message: GameMessage,
    ) -> Result<()> {
        i.update(client, |m| {
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
    type Residual = Flow<T>;

    fn from_output(output: Self::Output) -> Self {
        Self::Return(output)
    }

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            Self::Return(t) => ControlFlow::Continue(t),
            _ => ControlFlow::Break(self),
        }
    }
}

impl<T> FromResidual for Flow<T> {
    fn from_residual(residual: <Self as Try>::Residual) -> Self {
        residual
    }
}

#[async_trait]
pub trait Logic<T> {
    async fn logic(
        &mut self,
        client: &Discord,
        ui: &mut GameUI,
        i: Interaction<MessageComponent>,
    ) -> Flow<T>;
}

#[async_trait]
pub trait Game: Logic<()> + Sized + Send + 'static {
    const NAME: &'static str;
    const COLOR: u32;

    fn new() -> Self;
    fn lobby_msg_reply(&self) -> GameMessage;

    async fn start(
        client: &Discord,
        token: InteractionToken<ApplicationCommand>,
    ) -> Result<GameTask> {
        let me = Self::new();

        // send lobby message
        let msg = me.lobby_msg_reply();
        let msg = token
            .reply(client, |m| {
                m.embeds(vec![msg.embed]).components(msg.components)
            })
            .await?
            .get(client)
            .await?;

        // create thread
        let thread = msg.start_thread(client, Self::NAME.to_owned()).await?;

        // create task
        Ok(GameTask {
            ui: GameUI {
                channel: thread.id,
                lobby_msg: msg.id,
                state_msg: None,
            },
            game: Box::new(Self::new()),
        })
    }

    fn message(fields: Vec<Field>, components: Vec<ActionRow>) -> GameMessage {
        GameMessage {
            embed: Embed::default()
                .author(Author::new(Self::NAME.to_owned()))
                .fields(fields)
                .color(Self::COLOR),
            components,
        }
    }
}

pub struct Setup {
    pub options: Vec<(String, SetupOption)>,
}

impl Setup {
    pub fn render(&self) -> Vec<ActionRow> {
        self.options
            .iter()
            .map(|op| ActionRow {
                typ: MustBeU64::<1>,
                components: match op.1 {
                    SetupOption::MultiSelect(_) => todo!(),
                    SetupOption::Flags(_) => todo!(),
                    SetupOption::Number(min, max, val) => vec![
                        ActionRowComponent::Button {
                            style: ButtonStyle::Primary,
                            custom_id: Some(format!("{}_label", op.0)),
                            label: Some(op.0.clone()),
                            disabled: true,
                        },
                        ActionRowComponent::Button {
                            style: ButtonStyle::Primary,
                            custom_id: Some(format!("{}_sub", op.0)),
                            label: Some("<".to_owned()),
                            disabled: val <= min,
                        },
                        ActionRowComponent::Button {
                            style: ButtonStyle::Secondary,
                            custom_id: Some(format!("{}_value", op.0)),
                            label: Some(val.to_string()),
                            disabled: false,
                        },
                        ActionRowComponent::Button {
                            style: ButtonStyle::Primary,
                            custom_id: Some(format!("{}_add", op.0)),
                            label: Some(">".to_owned()),
                            disabled: val >= max,
                        },
                    ],
                },
            })
            .collect()
    }
}

#[async_trait]
impl Logic<()> for Setup {
    async fn logic(
        &mut self,
        client: &Discord,
        ui: &mut GameUI,
        i: Interaction<MessageComponent>,
    ) -> Flow<()> {
        // update state
        for option in self.options.iter_mut() {
            match &mut option.1 {
                SetupOption::MultiSelect(_) => todo!(),
                SetupOption::Flags(_) => todo!(),
                SetupOption::Number(min, max, val) => {
                    if i.data.custom_id == format!("{}_sub", option.0) {
                        *val = (*min).max(*val - 1);
                        break;
                    } else if i.data.custom_id == format!("{}_add", option.0) {
                        *val = (*max).min(*val + 1);
                        break;
                    }
                }
            }
        }

        // rerender
        let mut msg: GameMessage = i.data.message.into();
        msg.components = self.render();

        ui.update(client, i.token, msg).await.unwrap();
        Flow::Continue
    }
}

pub enum SetupOption {
    MultiSelect(Vec<(String, bool)>),
    Flags(Vec<(String, bool)>),
    Number(i32, i32, i32),
}
