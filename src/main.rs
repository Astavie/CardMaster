#![allow(incomplete_features)]
#![feature(try_trait_v2)]
#![feature(exhaustive_patterns)]
#![feature(adt_const_params)]

use std::{env, println};

use discord::command::{Param, StringOption};
use discord::interaction::{AnyInteraction, CreateReply, InteractionResource, Webhook};
use discord::request::Bot;
use discord::user;
use dotenv::dotenv;
use game::{Game, InteractionDispatcher};

use discord::application::{self, ApplicationResource};
use discord::command::CommandData;
use discord::command::{CommandResource, Commands};
use discord::gateway::Gateway;
use discord::gateway::GatewayEvent;
use discord::request::Result;

use crate::cah::CAH;

mod cah;
mod game;

async fn purge(commands: Commands, client: &Bot) -> Result<()> {
    if let Ok(commands) = commands.all(client).await {
        for command in commands {
            command.delete(client).await?;
        }
    }
    Ok(())
}

async fn on_command(i: AnyInteraction, d: &mut InteractionDispatcher, client: &Bot) -> Result<()> {
    match i {
        AnyInteraction::Command(command) => match command.data.name.as_str() {
            "ping" => {
                command
                    .token
                    .reply(&Webhook, CreateReply::default().content("hurb".into()))
                    .await?;
            }
            "play" => {
                let game = command.data.options[0].as_string().unwrap();
                let task = match game {
                    CAH::NAME => CAH::start(command.token, command.user, None),
                    _ => panic!("unknown game"),
                }
                .await?;
                d.register(task);
            }
            "playthread" => {
                let game = command.data.options[0].as_string().unwrap();
                let task = match game {
                    CAH::NAME => CAH::start(command.token, command.user, Some(client)),
                    _ => panic!("unknown game"),
                }
                .await?;
                d.register(task);
            }
            _ => {}
        },
        AnyInteraction::Component(comp) => d.dispatch(comp).await,
        AnyInteraction::Modal(submit) => {}
        AnyInteraction::MessageModal(submit) => {}
    };
    Ok(())
}

async fn run() -> Result<()> {
    // load dotenv
    dotenv().unwrap();
    let token = env::var("TOKEN").expect("Bot token TOKEN must be set");

    // connect
    let client = Bot::new(token);
    let application = application::Me.get(&client).await?;

    // list guilds
    let mut guilds = user::Me.get_guilds(&client).await?;
    println!("GUILDS");
    for guild in guilds.iter_mut() {
        purge(application.guild_commands(guild), &client).await?;
        println!(" - {}", guild.get_field(&client, |g| &g.name).await?);
    }

    // create commands
    purge(application.global_commands(), &client).await?;

    application
        .global_commands()
        .create(&client, CommandData::new("ping", "Replies with pong!"))
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
            CommandData::new("play", "Start a new game").options(vec![StringOption::new(
                "game",
                "What game to play",
            )
            .required()
            .choices(vec![Param::new(CAH::NAME, CAH::NAME)])
            .into()]),
        )
        .await?;

    application
        .global_commands()
        .create(
            &client,
            CommandData::new("playthread", "Start a new game within a thread").options(vec![
                StringOption::new("game", "What game to play")
                    .required()
                    .choices(vec![Param::new(CAH::NAME, CAH::NAME)])
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
