use async_trait::async_trait;
use discord::interaction::{AnyInteraction, CreateReply, InteractionResource, MessageComponent};
use dotenv_codegen::dotenv;
use futures_util::StreamExt;
use game::{Game, GameUI, InteractionDispatcher};

use crate::discord::application::{Application, ApplicationResource};
use crate::discord::command::CommandData;
use crate::discord::command::Commands;
use crate::discord::gateway::Gateway;
use crate::discord::gateway::GatewayEvent;
use crate::discord::interaction::Interaction;
use crate::discord::request::{Client, Result};
use crate::discord::resource::Deletable;
use crate::discord::Discord;

mod discord;
mod game;

const RUSTMASTER: &str = dotenv!("RUSTMASTER");
const CARDMASTER: &str = dotenv!("CARDMASTER");

async fn purge(commands: Commands, client: &impl Client) -> Result<()> {
    for command in commands.all(client).await? {
        command.delete(client).await?;
    }
    Ok(())
}

async fn on_command(
    client: &(impl Client + 'static),
    i: AnyInteraction,
    d: &mut InteractionDispatcher,
) -> Result<()> {
    match i {
        AnyInteraction::Command(command) => match command.data.name.as_str() {
            "ping" => {
                command
                    .token
                    .reply(client, |m| m.content("hurb".to_owned()))
                    .await?;
            }
            "play" => {
                let task = TestGame::start(client, command.token).await?;
                d.register(task);
            }
            _ => {}
        },
        AnyInteraction::Component(comp) => d.dispatch(comp).await,
    };
    Ok(())
}

struct TestGame;

#[async_trait]
impl Game for TestGame {
    fn new() -> Self {
        TestGame
    }

    fn lobby_msg_reply<'a>(msg: &'a mut CreateReply) -> &'a mut CreateReply {
        msg.content("# lobby".to_owned())
    }

    async fn logic(
        &mut self,
        _client: &impl Client,
        _ui: GameUI,
        _stream: tokio_stream::wrappers::ReceiverStream<Interaction<MessageComponent>>,
    ) -> Result<()> {
        Ok(())
    }

    fn name() -> &'static str {
        "Test Game"
    }
}

async fn run() -> Result<()> {
    // connect
    let client = Discord::new(RUSTMASTER);

    // create commands
    let application = Application::get(&client).await?;
    purge(application.global_commands(), &client).await?;

    application
        .global_commands()
        .create(
            &client,
            &CommandData::builder()
                .name("ping".to_owned())
                .description("Replies with pong!".to_owned())
                .build()
                .unwrap(),
        )
        .await?;
    application
        .global_commands()
        .create(
            &client,
            &CommandData::builder()
                .name("play".to_owned())
                .description("Start a new game".to_owned())
                .build()
                .unwrap(),
        )
        .await?;

    // create dispatch
    let mut dispatch = InteractionDispatcher::new();

    // gateway
    let handle = tokio::spawn(async move {
        let mut gateway = Gateway::connect(&client).await?;
        while let Some(event) = gateway.next().await {
            match event {
                GatewayEvent::InteractionCreate(i) => on_command(&client, i, &mut dispatch).await?,
                _ => {}
            }
        }
        gateway.close().await;
        Ok(())
    });

    handle.await.unwrap()?;
    Ok(())
}

#[tokio::main]
async fn main() {
    run().await.unwrap()
}
