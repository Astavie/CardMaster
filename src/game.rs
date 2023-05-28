use std::collections::HashMap;

use async_trait::async_trait;
use tokio::{
    sync::mpsc::{self, Sender},
    task::JoinHandle,
};
use tokio_stream::wrappers::ReceiverStream;

use crate::discord::{
    channel::Channel,
    interaction::{
        ApplicationCommand, CreateReply, Interaction, InteractionResource, InteractionToken,
        MessageComponent,
    },
    message::{Message, MessageIdentifier, MessageResource},
    request::{Client, Result},
    resource::{Resource, Snowflake},
};

pub struct InteractionDispatcher {
    lobbies: HashMap<Snowflake<Message>, Snowflake<Channel>>,
    channels: HashMap<Snowflake<Channel>, GameTask>,
}

pub struct GameTask {
    channel: Snowflake<Channel>,
    lobby: Snowflake<Message>,
    sender: Sender<Interaction<MessageComponent>>,
    handle: JoinHandle<Result<()>>,
}

impl InteractionDispatcher {
    pub fn new() -> Self {
        InteractionDispatcher {
            lobbies: HashMap::new(),
            channels: HashMap::new(),
        }
    }
    pub async fn dispatch(&self, i: Interaction<MessageComponent>) {
        if let Some(s) = self.lobbies.get(&i.data.message.id.snowflake()) {
            self.channels.get(s).unwrap().sender.send(i).await.unwrap();
        } else if let Some(s) = self.channels.get(&i.channel_id) {
            s.sender.send(i).await.unwrap();
        }
    }
    pub fn register(&mut self, task: GameTask) {
        self.lobbies.insert(task.lobby, task.channel);
        self.channels.insert(task.channel, task);
    }
}

pub struct GameUI {
    pub channel: Snowflake<Channel>,
    pub lobby_msg: MessageIdentifier,
    pub state_msg: Option<MessageIdentifier>,
}

#[async_trait]
pub trait Game: Sized + Send + 'static {
    fn new() -> Self;
    fn name() -> &'static str;
    fn lobby_msg_reply<'a>(msg: &'a mut CreateReply) -> &'a mut CreateReply;

    async fn logic(
        &mut self,
        client: &impl Client,
        ui: GameUI,
        stream: ReceiverStream<Interaction<MessageComponent>>,
    ) -> Result<()>;

    async fn start(
        client: &(impl Client + 'static),
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
        let thread = msg.start_thread(client, Self::name().to_owned()).await?;

        // create task
        let ui = GameUI {
            channel: thread.id,
            lobby_msg: msg.id,
            state_msg: None,
        };

        let mut game = Self::new();
        let (tx, rx) = mpsc::channel(16);

        let client = client.clone();
        let handle = tokio::spawn(async move {
            let future = game.logic(&client, ui, ReceiverStream::new(rx));
            future.await
        });

        Ok(GameTask {
            channel: thread.id,
            lobby: msg.id.snowflake(),
            sender: tx,
            handle,
        })
    }
}
