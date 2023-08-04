#![allow(incomplete_features)]
#![feature(try_trait_v2)]
#![feature(exhaustive_patterns)]
#![feature(adt_const_params)]

use std::{env, println};

use async_trait::async_trait;
use discord::command::{Param, StringOption};
use discord::interaction::{AnyInteraction, InteractionResource, MessageComponent, Webhook};
use discord::request::Discord;
use discord::user::{self, User};
use dotenv::dotenv;
use futures_util::StreamExt;
use game::{Flow, Game, GameMessage, GameUI, InteractionDispatcher, Logic};

use discord::application::{self, ApplicationResource};
use discord::command::CommandData;
use discord::command::Commands;
use discord::gateway::Gateway;
use discord::gateway::GatewayEvent;
use discord::interaction::Interaction;
use discord::request::Result;
use discord::resource::{Creatable, Deletable, Resource};

use crate::cah::CAH;

mod cah;
mod game;

async fn purge(commands: Commands, client: &Discord) -> Result<()> {
    if let Ok(commands) = commands.get(client).await {
        for command in commands {
            command.delete(client).await?;
        }
    }
    Ok(())
}

async fn on_command(
    i: AnyInteraction,
    d: &mut InteractionDispatcher,
    client: &Discord,
) -> Result<()> {
    match i {
        AnyInteraction::Command(command) => match command.data.name.as_str() {
            "ping" => {
                command
                    .token
                    .reply(&Webhook, |m| m.content("hurb".into()))
                    .await?;
            }
            "play" => {
                let game = command.data.options[0].as_string().unwrap();
                let task = match game {
                    TestGame::NAME => TestGame::start(command.token, command.user, None),
                    CAH::NAME => CAH::start(command.token, command.user, None),
                    _ => panic!("unknown game"),
                }
                .await?;
                d.register(task);
            }
            "playthread" => {
                let game = command.data.options[0].as_string().unwrap();
                let task = match game {
                    TestGame::NAME => TestGame::start(command.token, command.user, Some(client)),
                    CAH::NAME => CAH::start(command.token, command.user, Some(client)),
                    _ => panic!("unknown game"),
                }
                .await?;
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
impl Logic for TestGame {
    type Return = ();
    async fn logic(&mut self, _ui: &mut GameUI, _i: Interaction<MessageComponent>) -> Flow<()> {
        Flow::Return(())
    }
}

#[async_trait]
impl Game for TestGame {
    const NAME: &'static str = "Test";
    const COLOR: u32 = 0xFFFFFF;

    fn new(_user: User) -> Self {
        TestGame
    }

    fn lobby_msg_reply(&self) -> GameMessage {
        GameMessage::new(Vec::new(), Vec::new())
    }
}

async fn run() -> Result<()> {
    // load dotenv
    dotenv().unwrap();
    let token = env::var("CARDMASTER").expect("Bot token CARDMASTER must be set");

    // connect
    let client = Discord::new(token);
    let application = application::Me.get(&client).await?;

    // list guilds
    let mut guilds = user::Me.get_guilds(&client).await?;
    println!("GUILDS");
    for guild in guilds.iter_mut() {
        purge(application.guild_commands(guild), &client).await?;
        println!(" - {}", guild.get_name(&client).await?);
    }

    // create commands
    purge(application.global_commands(), &client).await?;

    application
        .global_commands()
        .create(&client, &CommandData::new("ping", "Replies with pong!"))
        .await?;

    // application
    //     .global_commands()
    //     .create(
    //         &client,
    //         &CommandData::new("birthday", "Sets user birthday").options(vec![StringOption::new(
    //             "birthday",
    //             "Your birthday",
    //         )
    //         .required()
    //         .into()]),
    //     )
    //     .await?;

    application
        .global_commands()
        .create(
            &client,
            &CommandData::new("play", "Start a new game").options(vec![StringOption::new(
                "game",
                "What game to play",
            )
            .required()
            .choices(vec![
                Param::new(TestGame::NAME, TestGame::NAME),
                Param::new(CAH::NAME, CAH::NAME),
            ])
            .into()]),
        )
        .await?;

    application
        .global_commands()
        .create(
            &client,
            &CommandData::new("playthread", "Start a new game within a thread").options(vec![
                StringOption::new("game", "What game to play")
                    .required()
                    .choices(vec![
                        Param::new(TestGame::NAME, TestGame::NAME),
                        Param::new(CAH::NAME, CAH::NAME),
                    ])
                    .into(),
            ]),
        )
        .await?;

    // create dispatch
    let mut dispatch = InteractionDispatcher::new();

    // gateway
    let mut gateway = Gateway::connect(&client).await?;
    while let Some(event) = gateway.next().await {
        match event {
            GatewayEvent::InteractionCreate(i) => on_command(i, &mut dispatch, &client).await?,
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
