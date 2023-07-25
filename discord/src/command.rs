use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::resource::Endpoint;

use super::request::Discord;
use super::{
    application::Application,
    guild::Guild,
    request::{Request, Result},
    resource::{Resource, Snowflake},
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
        self.create_request(command).request(client).await
    }

    pub fn all_request(&self) -> Request<Vec<Command>> {
        Request::get(self.uri())
    }
    pub async fn all(&self, client: &Discord) -> Result<Vec<Command>> {
        self.all_request().request(client).await
    }
}

impl Endpoint for CommandIdentifier {
    type Result = Command;
    type Delete = ();
    fn uri(&self) -> String {
        format!("{}/{}", self.command_pool.uri(), self.command_id)
    }
}

impl Resource for Command {
    type Endpoint = CommandIdentifier;
    fn endpoint(&self) -> &Self::Endpoint {
        &self.id
    }
}
