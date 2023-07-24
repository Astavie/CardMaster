use std::{
    collections::HashMap,
    convert, format,
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
        ActionRow, ActionRowComponent, Author, Button, ButtonStyle, Embed, Field, Message,
        MessageIdentifier, MessageResource, SelectOption, TextSelectMenu,
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
    game: Box<dyn Logic<()>>,
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
        i: InteractionToken<MessageComponent>,
        message: GameMessage,
    ) -> Result<()> {
        i.reply(|m| {
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
        i.update(|m| m.embeds(vec![message.embed]).components(message.components))
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
    type Residual = Flow<convert::Infallible>;

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
    fn from_residual(residual: Flow<convert::Infallible>) -> Self {
        match residual {
            Flow::Continue => Flow::Continue,
            Flow::Exit => Flow::Exit,
        }
    }
}

impl<T> FromResidual<Option<convert::Infallible>> for Flow<T> {
    fn from_residual(residual: Option<convert::Infallible>) -> Self {
        match residual {
            None => Flow::Continue,
        }
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
pub trait Game: Logic<()> + Sized + 'static {
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
            .reply(|m| m.embeds(vec![msg.embed]).components(msg.components))
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
                .author(Author::new(Self::NAME))
                .fields(fields)
                .color(Self::COLOR),
            components,
        }
    }
}

pub struct Setup {
    pub options: Vec<(String, SetupOption)>,
}

const B64_TABLE: [char; 64] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l',
    'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '+', '/',
];

impl Setup {
    pub fn render(&self) -> Vec<ActionRow> {
        self.options
            .iter()
            .enumerate()
            .map(|(oi, (name, option))| ActionRow {
                typ: MustBeU64::<1>,
                components: match *option {
                    SetupOption::MultiSelect(ref menu) => {
                        assert!(menu.len() <= 64);
                        vec![ActionRowComponent::TextSelectMenu(TextSelectMenu {
                            custom_id: format!("{}", B64_TABLE[oi]),
                            options: menu
                                .iter()
                                .enumerate()
                                .map(|(i, &(ref name, enabled))| SelectOption {
                                    label: name.clone(),
                                    value: format!("{}", B64_TABLE[i]),
                                    description: None,
                                    default: enabled,
                                })
                                .collect(),
                            placeholder: Some(name.clone()),
                            min_values: 0,
                            max_values: menu.len(),
                            disabled: false,
                        })]
                    }
                    SetupOption::Flags(ref menu) => {
                        assert!(menu.len() <= 4);
                        let mut buttons = vec![ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: format!("_label_{}", B64_TABLE[oi]),
                            label: Some(name.clone()),
                            disabled: true,
                        })];
                        buttons.extend(menu.iter().enumerate().map(|(i, &(ref name, enabled))| {
                            ActionRowComponent::Button(Button::Action {
                                style: if enabled {
                                    // Green
                                    ButtonStyle::Success
                                } else {
                                    // Gray
                                    ButtonStyle::Secondary
                                },
                                custom_id: format!("{}{}", B64_TABLE[oi], B64_TABLE[i]),
                                label: Some(name.clone()),
                                disabled: false,
                            })
                        }));
                        buttons
                    }
                    SetupOption::Number(min, max, val) => vec![
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: format!("_label_{}", B64_TABLE[oi]),
                            label: Some(name.clone()),
                            disabled: true,
                        }),
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: format!("{}d", B64_TABLE[oi]),
                            label: Some("<".to_owned()),
                            disabled: val <= min,
                        }),
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Secondary,
                            custom_id: format!("{}v", B64_TABLE[oi]),
                            label: Some(val.to_string()),
                            disabled: false,
                        }),
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: format!("{}i", B64_TABLE[oi]),
                            label: Some(">".to_owned()),
                            disabled: val >= max,
                        }),
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
        it: Interaction<MessageComponent>,
    ) -> Flow<()> {
        // update state
        let mut chars = it.data.custom_id.chars();
        let ob = chars.next()?;
        let oi = B64_TABLE.iter().position(|&c| c == ob)?;
        let option = &mut self.options.get_mut(oi)?.1;

        match *option {
            SetupOption::MultiSelect(ref mut menu) => {
                for (_, option) in menu.iter_mut() {
                    *option = false;
                }
                for select in it.data.values {
                    let Some(b) = select.value.chars().next() else { continue };
                    let Some(i) = B64_TABLE.iter().position(|&c| c == b) else { continue };
                    let Some(option) = menu.get_mut(i).map(|(_, b)| b) else { continue };
                    *option = true;
                }
            }
            SetupOption::Flags(ref mut menu) => {
                let b = chars.next()?;
                let i = B64_TABLE.iter().position(|&c| c == b)?;
                let option = &mut menu.get_mut(i)?.1;
                *option = !*option;
            }
            SetupOption::Number(min, max, ref mut val) => match chars.next()? {
                'd' if *val > min => {
                    *val = *val - 1;
                }
                'i' if *val < max => {
                    *val = *val + 1;
                }
                _ => None?,
            },
        }

        // rerender
        let mut msg: GameMessage = it.data.message.into();
        msg.components = self.render();

        ui.update(it.token, msg).await.unwrap();
        Flow::Continue
    }
}

pub enum SetupOption {
    MultiSelect(Vec<(String, bool)>),
    Flags(Vec<(String, bool)>),
    Number(i32, i32, i32),
}
