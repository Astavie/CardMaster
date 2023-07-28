use std::{
    convert, format,
    ops::{ControlFlow, FromResidual, Try},
};

use async_trait::async_trait;
use monostate::MustBeU64;

use discord::{
    interaction::{
        ApplicationCommand, Interaction, InteractionResource, InteractionResponseIdentifier,
        InteractionToken, MessageComponent, ReplyFlag, Webhook,
    },
    message::{
        ActionRow, ActionRowComponent, Author, Button, ButtonStyle, Embed, Field, Message,
        SelectOption, TextSelectMenu,
    },
    request::Result,
    resource::{Patchable, Resource, Snowflake},
    user::User,
};

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
        let Some(pos) = self
            .games
            .iter()
            .position(|s| s.ui.msg_id == msg) else { return };

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
    pub msg: InteractionResponseIdentifier,
    pub msg_id: Snowflake<Message>,
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
    pub async fn push(&mut self, message: GameMessage) -> Result<()> {
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
        let msg = me.lobby_msg_reply();
        let id = token
            .reply(&Webhook, |m| {
                m.embeds(vec![msg.embed]).components(msg.components)
            })
            .await?;
        let msg = id.get(&Webhook).await?;

        // create task
        Ok(GameTask {
            ui: GameUI {
                msg: id,
                msg_id: msg.id.snowflake(),
            },
            game: Box::new(me),
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
        assert!(self.options.len() <= 4);
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

impl Setup {
    pub fn update(&mut self, it: &Interaction<MessageComponent>) -> Flow<()> {
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
                for select in it.data.values.iter() {
                    let Some(b) = select.chars().next() else { continue };
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

        Flow::Return(())
    }
}

#[async_trait]
impl Logic for Setup {
    type Return = ();

    async fn logic(&mut self, ui: &mut GameUI, it: Interaction<MessageComponent>) -> Flow<()> {
        self.update(&it)?;

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
