use std::collections::HashMap;

use async_trait::async_trait;

use crate::discord::{
    channel::Channel,
    interaction::{
        ApplicationCommand, CreateReply, Interaction, InteractionResource, InteractionToken,
        MessageComponent,
    },
    message::{Message, MessageIdentifier, MessageResource},
    request::{Discord, Result},
    resource::{Resource, Snowflake},
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

        if result.is_ok() {
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

#[async_trait]
pub trait Logic<T> {
    async fn logic(
        &mut self,
        client: &Discord,
        ui: &mut GameUI,
        i: Interaction<MessageComponent>,
    ) -> Result<T>;
}

#[async_trait]
pub trait Game: Logic<()> + Sized + Send + 'static {
    const NAME: &'static str;

    fn new() -> Self;
    fn lobby_msg_reply<'a>(msg: &'a mut CreateReply) -> &'a mut CreateReply;

    async fn start(
        client: &Discord,
        token: InteractionToken<ApplicationCommand>,
    ) -> Result<GameTask> {
        // send lobby message
        let mut msg = CreateReply::default();
        Self::lobby_msg_reply(&mut msg);

        let msg = token
            .reply(client, |a| {
                *a = msg;
                a
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
}
