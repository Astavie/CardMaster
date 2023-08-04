use async_trait::async_trait;
use derive_setters::Setters;
use monostate::{MustBe, MustBeU64};
use partial_id::Partial;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::resource::Endpoint;

use super::request::{Discord, Request, Result};
use super::{
    channel::Channel,
    resource::{Resource, Snowflake},
    user::PartialUser,
};

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct MessageIdentifier {
    channel_id: Snowflake<Channel>,

    #[serde(rename = "id")]
    message_id: Snowflake<Message>,
}

impl MessageIdentifier {
    pub fn snowflake(&self) -> Snowflake<Message> {
        self.message_id
    }
}

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(flatten)]
    pub id: MessageIdentifier,

    pub author: PartialUser,
    pub content: String,

    #[serde(default)]
    pub embeds: Vec<Embed>,
    #[serde(default)]
    pub components: Vec<ActionRow>,
}

#[derive(Default, Setters, Serialize)]
#[setters(strip_option)]
pub struct CreateMessage {
    content: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    embeds: Vec<Embed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    components: Vec<ActionRow>,
}

#[derive(Default, Setters, Serialize)]
#[setters(strip_option)]
pub struct PatchMessage {
    content: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    embeds: Vec<Embed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    components: Vec<ActionRow>,
}

#[derive(Debug, Default, Setters, Serialize, Deserialize)]
#[setters(strip_option)]
pub struct Embed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub color: Option<u32>,
    pub author: Option<Author>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fields: Vec<Field>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionRow {
    #[serde(rename = "type")]
    typ: MustBe!(1u64),
    pub components: Vec<ActionRowComponent>,
}

impl ActionRow {
    pub fn new(components: Vec<ActionRowComponent>) -> Self {
        Self {
            typ: MustBeU64,
            components,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ButtonStyle {
    Primary = 1,
    Secondary = 2,
    Success = 3,
    Danger = 4,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Button {
    Action {
        style: ButtonStyle,
        custom_id: String,
        label: Option<String>,
        #[serde(skip_serializing_if = "std::ops::Not::not", default)]
        disabled: bool,
    },
    Link {
        style: MustBe!(5u64),
        url: String,
        label: Option<String>,
        #[serde(skip_serializing_if = "std::ops::Not::not", default)]
        disabled: bool,
    },
}

const fn _default_1() -> usize {
    1
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextSelectMenu {
    pub custom_id: String,
    pub options: Vec<SelectOption>,
    pub placeholder: Option<String>,
    #[serde(default = "_default_1")]
    pub min_values: usize,
    #[serde(default = "_default_1")]
    pub max_values: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub disabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionRowComponent {
    #[serde(rename = 2)]
    Button(Button),
    #[serde(rename = 3)]
    TextSelectMenu(TextSelectMenu),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelectOption {
    pub label: String,
    pub value: String,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub default: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
}

impl Author {
    pub fn new<S>(name: S) -> Self
    where
        S: Into<String>,
    {
        Self { name: name.into() }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub value: String,

    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub inline: bool,
}

impl Field {
    pub fn new<S1, S2>(name: S1, value: S2) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Self {
            name: name.into(),
            value: value.into(),
            inline: false,
        }
    }
    pub fn inlined<S1, S2>(name: S1, value: S2) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Self {
            name: name.into(),
            value: value.into(),
            inline: true,
        }
    }
}

#[derive(Serialize)]
struct CreateThread {
    name: String,
}

#[async_trait]
pub trait MessageResource: Resource<Endpoint = MessageIdentifier> {
    fn channel(&self) -> Snowflake<Channel> {
        self.endpoint().channel_id
    }

    fn start_thread_request(&self, name: String) -> Request<Channel> {
        let id = self.endpoint();
        Request::post(
            format!(
                "/channels/{}/messages/{}/threads",
                id.channel_id.as_int(),
                id.message_id.as_int()
            ),
            &CreateThread { name },
        )
    }
    async fn start_thread(&self, client: &Discord, name: String) -> Result<Channel> {
        self.start_thread_request(name).request(client).await
    }
}

impl<T> MessageResource for T where T: Resource<Endpoint = MessageIdentifier> {}

impl Endpoint for MessageIdentifier {
    type Result = Message;
    type Patch = PatchMessage;
    type Delete = ();

    fn uri(&self) -> String {
        format!(
            "/channels/{}/messages/{}",
            self.channel_id.as_int(),
            self.message_id.as_int()
        )
    }
}

impl Resource for Message {
    type Endpoint = MessageIdentifier;
    fn endpoint(&self) -> &Self::Endpoint {
        &self.id
    }
}
impl Resource for PartialMessage {
    type Endpoint = MessageIdentifier;
    fn endpoint(&self) -> &Self::Endpoint {
        &self.id
    }
}
