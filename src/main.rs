use async_trait::async_trait;
use cah::CAH;
use discord::command::{Param, StringOption};
use discord::interaction::{AnyInteraction, CreateReply, InteractionResource, MessageComponent};
use discord::request::Discord;
use dotenv_codegen::dotenv;
use futures_util::StreamExt;
use game::{Game, GameUI, InteractionDispatcher, Logic};

use crate::discord::application::{Application, ApplicationResource};
use crate::discord::command::CommandData;
use crate::discord::command::Commands;
use crate::discord::gateway::Gateway;
use crate::discord::gateway::GatewayEvent;
use crate::discord::interaction::Interaction;
use crate::discord::request::Result;
use crate::discord::resource::Deletable;

mod cah;
mod discord;
mod game;

const RUSTMASTER: &str = dotenv!("RUSTMASTER");
const CARDMASTER: &str = dotenv!("CARDMASTER");

async fn purge(commands: Commands, client: &Discord) -> Result<()> {
    for command in commands.all(client).await? {
        command.delete(client).await?;
    }
    Ok(())
}

async fn on_command(
    client: &Discord,
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
                let game = command.data.options[0].as_string().unwrap();
                let task = match game {
                    TestGame::NAME => TestGame::start(client, command.token),
                    CAH::NAME => CAH::start(client, command.token),
                    _ => panic!("unknown game"),
                }
                .await?;
                d.register(task);
            }
            _ => {}
        },
        AnyInteraction::Component(comp) => d.dispatch(client, comp).await,
    };
    Ok(())
}

struct TestGame;

#[async_trait]
impl Logic<()> for TestGame {
    async fn logic(
        &mut self,
        _client: &Discord,
        _ui: &mut GameUI,
        _i: Interaction<MessageComponent>,
    ) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl Game for TestGame {
    const NAME: &'static str = "Test";

    fn new() -> Self {
        TestGame
    }

    fn lobby_msg_reply<'a>(msg: &'a mut CreateReply) -> &'a mut CreateReply {
        msg.content("# lobby".to_owned())
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
                .options(vec![StringOption::builder()
                    .name("game".to_owned())
                    .description("What game to play".to_owned())
                    .required(true)
                    .choices(vec![
                        Param::new(TestGame::NAME.to_owned(), TestGame::NAME.to_owned()),
                        Param::new(CAH::NAME.to_owned(), CAH::NAME.to_owned()),
                    ])
                    .build()
                    .unwrap()
                    .into()])
                .build()
                .unwrap(),
        )
        .await?;

    // create dispatch
    let mut dispatch = InteractionDispatcher::new();

    // gateway
    let mut gateway = Gateway::connect(&client).await?;
    while let Some(event) = gateway.next().await {
        match event {
            GatewayEvent::InteractionCreate(i) => on_command(&client, i, &mut dispatch).await?,
            _ => {}
        }
    }
    gateway.close().await;
    Ok(())
}

#[tokio::main]
async fn main() {
    run().await.unwrap()
}
