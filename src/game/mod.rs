use std::{collections::HashMap, str::FromStr, unreachable};

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

use self::widget::Event;

pub mod widget;

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
    game: Box<dyn Logic>,
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
        let is_done = task.game.logic(&mut task.ui, i).await;

        if is_done {
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
    panel: &'static str,

    replies: HashMap<Snowflake<Message>, (&'static str, InteractionResponseIdentifier)>,
}

#[derive(Default)]
pub struct GameMessage {
    pub fields: Vec<Field>,
    pub components: Vec<ActionRow>,
}

impl GameMessage {
    pub fn new(fields: Vec<Field>, components: Vec<ActionRow>) -> Self {
        Self { fields, components }
    }
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.components.is_empty()
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

impl GameUI {
    pub async fn edit(&self, id: Snowflake<Message>, msg: GameMessage) {
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
                .1
                .patch(
                    &Webhook,
                    PatchMessage::default()
                        .embeds(vec![Embed::default().fields(msg.fields)])
                        .components(msg.components),
                )
                .await
                .unwrap();
        }
    }
    pub async fn reply_panel<P: Into<&'static str>>(
        &mut self,
        i: Interaction<MessageComponent>,
        msg: GameMessage,
        panel: P,
    ) {
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
        self.replies.insert(id, (panel.into(), response));
    }
    pub async fn reply(&mut self, i: Interaction<MessageComponent>, msg: GameMessage) {
        // we do not sign replies
        i.reply(
            &Webhook,
            CreateReply::default()
                .embeds(vec![Embed::default().fields(msg.fields)])
                .components(msg.components)
                .flags(ReplyFlag::Ephemeral.into()),
        )
        .await
        .unwrap();
    }
    pub async fn update_panel<P: Into<&'static str>>(
        &mut self,
        i: Interaction<MessageComponent>,
        msg: GameMessage,
        panel: P,
    ) {
        if i.data.message.id.snowflake() == self.msg_id {
            // sign if we are updating the base message
            self.panel = panel.into();
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
            self.replies
                .get_mut(&i.data.message.id.snowflake())
                .unwrap()
                .0 = panel.into();
            i.update(
                &Webhook,
                CreateUpdate::default()
                    .embeds(vec![Embed::default().fields(msg.fields)])
                    .components(msg.components),
            )
            .await
            .unwrap();
        }
    }
    pub async fn update(&mut self, i: Interaction<MessageComponent>, msg: GameMessage) {
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
    }
    pub async fn delete_replies(&mut self) {
        let _ = join_all(self.replies.drain().map(|(_, (_, id))| id.delete(&Webhook))).await;
    }
}

#[async_trait]
trait Logic {
    async fn logic(&mut self, ui: &mut GameUI, i: Interaction<MessageComponent>) -> bool;
}

#[async_trait]
impl<T> Logic for T
where
    T: Game + Send,
{
    async fn logic(&mut self, ui: &mut GameUI, interaction: Interaction<MessageComponent>) -> bool {
        let panel = {
            if interaction.data.message.id.snowflake() == ui.msg_id {
                ui.panel
            } else {
                ui.replies[&interaction.data.message.id.snowflake()].0
            }
        };
        let panel = match T::Panel::from_str(panel) {
            Ok(panel) => panel,
            Err(_) => unreachable!(),
        };
        let mut action = T::Action::from_str(&interaction.data.custom_id).ok();

        let mut panel_msg = GameMessage::default();
        if action.is_none() {
            action = self.create_panel(&mut panel_msg, &Event::component(&interaction), panel);
        }

        match action {
            Some(action) => {
                let response = self.on_action(action, panel, &interaction.user);
                match response {
                    ActionResponse::RefreshMain => {
                        // update panel if it should be updated
                        if !panel_msg.is_empty() {
                            ui.update(interaction, panel_msg).await;
                        }

                        // edit main panel
                        let mut msg = GameMessage::default();
                        self.create_panel(
                            &mut msg,
                            &Event::none(),
                            match T::Panel::from_str(ui.panel) {
                                Ok(panel) => panel,
                                Err(_) => unreachable!(),
                            },
                        );
                        ui.edit(ui.msg_id, msg).await;
                        false
                    }
                    ActionResponse::RefreshPanel => {
                        // delete replies if we are refreshing the main panel
                        if interaction.data.message.id.snowflake() == ui.msg_id {
                            ui.delete_replies().await;
                        }

                        // update panel
                        let mut msg = GameMessage::default();
                        self.create_panel(&mut msg, &Event::none(), panel);
                        ui.update_panel(interaction, msg, panel).await;
                        false
                    }
                    ActionResponse::NewPanel(panel) => {
                        // create new panel
                        let mut msg = GameMessage::default();
                        self.create_panel(&mut msg, &Event::none(), panel);
                        ui.reply_panel(interaction, msg, panel).await;
                        false
                    }
                    ActionResponse::Reply(msg) => {
                        // send quick message
                        ui.reply(interaction, msg).await;
                        false
                    }
                    ActionResponse::Exit(msg) => {
                        // exit with message
                        ui.delete_replies().await;
                        ui.update(interaction, msg).await;
                        true
                    }
                    ActionResponse::None => {
                        // update panel if it should be updated
                        if !panel_msg.is_empty() {
                            ui.update(interaction, panel_msg).await;
                        }
                        false
                    }
                }
            }
            None => {
                // no actions
                ui.update(interaction, panel_msg).await;
                false
            }
        }
    }
}

#[macro_export]
macro_rules! enum_str {
    ( $name:ident : $first:ident $(, $elem:ident)* ) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            #[default]
            $first,
            $($elem),*
        }
        impl std::str::FromStr for $name {
            type Err = ();
            fn from_str(s: &str) -> Result<Self, ()> {
                match s {
                    stringify!($first) => Ok(Self::$first),
                    $(stringify!($elem) => Ok(Self::$elem),)*
                    _ => Err(()),
                }
            }
        }
        impl<'a> From<$name> for &'a str {
            fn from(value: $name) -> &'a str {
                match value {
                    $name::$first => stringify!($first),
                    $($name::$elem => stringify!($elem),)*
                }
            }
        }
    }
}

pub enum ActionResponse<Panel> {
    RefreshMain,

    RefreshPanel,
    NewPanel(Panel),

    Reply(GameMessage),
    Exit(GameMessage),

    None,
}

#[async_trait]
pub trait Game: Sized + 'static {
    type Action: FromStr + Into<&'static str> + Send;
    type Panel: FromStr + Into<&'static str> + Send + Copy + Default;

    const NAME: &'static str;
    const COLOR: u32;

    fn new(user: User) -> Self;

    fn create_panel(
        &mut self,
        msg: &mut GameMessage,
        event: &Event,
        panel: Self::Panel,
    ) -> Option<Self::Action>;

    fn on_action(
        &mut self,
        action: Self::Action,
        panel: Self::Panel,
        user: &User,
    ) -> ActionResponse<Self::Panel>;

    async fn start(
        token: InteractionToken<ApplicationCommand>,
        user: User,
        thread: Option<&Discord>,
    ) -> Result<GameTask> {
        let mut me = Self::new(user);

        // send lobby message
        let mut msg = GameMessage::default();
        me.create_panel(&mut msg, &Event::none(), Self::Panel::default());

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
                panel: Self::Panel::default().into(),
                replies: HashMap::new(),
            },
            game: Box::new(me),
        })
    }
}
