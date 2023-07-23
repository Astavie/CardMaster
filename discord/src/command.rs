use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use super::request::Discord;
use super::{
    application::Application,
    guild::Guild,
    request::{Request, Result},
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

#[derive(Debug, Deserialize, Serialize, Setters)]
pub struct CommandData {
    #[setters(skip)]
    pub name: String,
    #[setters(skip)]
    pub description: String,

    #[serde(rename = "type", default)]
    pub input_type: CommandType,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub options: Vec<CommandOption>,
}

impl CommandData {
    pub fn new<S1, S2>(name: S1, description: S2) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Self {
            name: name.into(),
            description: description.into(),
            input_type: CommandType::ChatInput,
            options: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum CommandOption {
    #[serde(rename = 3)]
    String(StringOption),
}

#[derive(Debug, Deserialize, Serialize, Setters)]
pub struct StringOption {
    #[setters(skip)]
    pub name: String,
    #[setters(skip)]
    pub description: String,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub choices: Vec<Param<String>>,

    #[serde(default)]
    #[setters(bool)]
    pub required: bool,
}

impl StringOption {
    pub fn new<S1, S2>(name: S1, description: S2) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Self {
            name: name.into(),
            description: description.into(),
            choices: Vec::new(),
            required: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Param<T> {
    pub name: String,
    pub value: T,
}

impl<T> Param<T> {
    pub fn new<S>(name: S, value: T) -> Param<T>
    where
        S: Into<String>,
    {
        Param {
            name: name.into(),
            value,
        }
    }
}

impl From<StringOption> for CommandOption {
    fn from(value: StringOption) -> Self {
        Self::String(value)
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
    pub async fn create(&self, client: &Discord, command: &CommandData) -> Result<Command> {
        client.request(self.create_request(command)).await
    }

    pub fn all_request(&self) -> Request<Vec<Command>> {
        Request::get(self.uri())
    }
    pub async fn all(&self, client: &Discord) -> Result<Vec<Command>> {
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
