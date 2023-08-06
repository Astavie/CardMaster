use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::request::Discord;
use crate::request::HttpRequest;
use crate::resource::resource;
use crate::resource::Endpoint;

use super::{application::Application, guild::Guild, resource::Snowflake};

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
    #[serde(rename = 1)]
    SubCommand,
    #[serde(rename = 2)]
    SubCommandGroup,
    #[serde(rename = 3)]
    String(StringOption),
    #[serde(rename = 4)]
    Integer,
    #[serde(rename = 5)]
    Boolean,
    #[serde(rename = 6)]
    User,
    #[serde(rename = 7)]
    Channel,
    #[serde(rename = 8)]
    Role,
    #[serde(rename = 9)]
    Mentionable,
    #[serde(rename = 10)]
    Number,
    #[serde(rename = 11)]
    Attachment,
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
    pub fn new<S, I>(name: S, value: I) -> Param<T>
    where
        S: Into<String>,
        I: Into<T>,
    {
        Param {
            name: name.into(),
            value: value.into(),
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
}

impl Endpoint for Commands {
    fn uri(&self) -> String {
        if let Some(guild) = self.guild_id {
            format!(
                "/applications/{}/guilds/{}/commands",
                self.application_id.as_int(),
                guild.as_int()
            )
        } else {
            format!("/applications/{}/commands", self.application_id.as_int())
        }
    }
}

impl Endpoint for CommandIdentifier {
    fn uri(&self) -> String {
        format!("{}/{}", self.command_pool.uri(), self.command_id.as_int())
    }
}

resource! {
    CommandsResource as Commands;
    use Discord;

    fn all(&self) -> Vec<Command> {
        HttpRequest::get(self.endpoint().uri())
    }
    fn create(&self, data: CommandData) -> Command {
        HttpRequest::post(self.endpoint().uri(), &data)
    }
}

resource! {
    CommandResource as CommandIdentifier;
    use Discord;

    fn get(&self) -> Command {
        HttpRequest::get(self.endpoint().uri())
    }
    fn delete(mut self) -> () {
        HttpRequest::delete(self.endpoint().uri())
    }
}

impl CommandResource for Command {
    fn endpoint(&self) -> &CommandIdentifier {
        &self.id
    }
}
