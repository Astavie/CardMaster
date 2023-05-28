use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::discord::{
    application::Application,
    guild::Guild,
    request::{Client, Request, Result},
    resource::{Deletable, Resource, Snowflake},
};

#[derive(Debug, Deserialize, Copy, Clone)]
pub struct Commands {
    application_id: Snowflake<Application>,
    guild_id: Option<Snowflake<Guild>>,
}

#[derive(
    Debug,
    Serialize_repr,
    Deserialize_repr,
    PartialEq,
    Eq,
    Default,
    Copy,
    Clone
)]
#[repr(u8)]
pub enum CommandType {
    #[default]
    ChatInput = 1,
    User = 2,
    Message = 3,
}

#[derive(Debug, Deserialize, Serialize, Builder)]
pub struct CommandData {
    pub name: String,
    pub description: String,

    #[serde(rename = "type", default)]
    #[builder(default)]
    pub input_type: CommandType,
}

impl CommandData {
    pub fn builder() -> CommandDataBuilder {
        CommandDataBuilder::default()
    }
}

#[derive(Debug, Deserialize, Copy, Clone)]
pub struct CommandIdentifier {
    #[serde(flatten)]
    command_pool: Commands,
    #[serde(rename = "id")]
    command_id: Snowflake<Command>,
}

#[derive(Debug, Deserialize)]
pub struct Command {
    #[serde(flatten)]
    pub id: CommandIdentifier,
    #[serde(flatten)]
    pub data: CommandData,
}

impl Commands {
    pub(crate) fn new(app: Snowflake<Application>, guild: Option<Snowflake<Guild>>) -> Self {
        Self {
            application_id: app,
            guild_id: guild,
        }
    }

    fn uri(&self) -> String {
        if let Some(guild) = self.guild_id {
            format!(
                "/applications/{}/guilds/{}/commands",
                self.application_id, guild
            )
        } else {
            format!("/applications/{}/commands", self.application_id)
        }
    }

    pub fn create_request(&self, command: &CommandData) -> Request<Command> {
        Request::post(self.uri(), command)
    }
    pub async fn create(&self, client: &impl Client, command: &CommandData) -> Result<Command> {
        client.request(self.create_request(command)).await
    }

    pub fn all_request(&self) -> Request<Vec<Command>> {
        Request::get(self.uri())
    }
    pub async fn all(&self, client: &impl Client) -> Result<Vec<Command>> {
        client.request(self.all_request()).await
    }
}

pub trait CommandResource {
    fn endpoint(&self) -> CommandIdentifier;
}

impl CommandResource for CommandIdentifier {
    fn endpoint(&self) -> CommandIdentifier {
        self.clone()
    }
}

impl CommandResource for Command {
    fn endpoint(&self) -> CommandIdentifier {
        self.id
    }
}

impl<T> Resource<Command> for T
where
    T: CommandResource,
{
    fn uri(&self) -> String {
        format!(
            "{}/{}",
            self.endpoint().command_pool.uri(),
            self.endpoint().command_id
        )
    }
}

impl<T> Deletable<Command> for T where T: CommandResource {}
