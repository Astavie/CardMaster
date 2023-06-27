use async_trait::async_trait;
use derive_setters::Setters;
use monostate::MustBe;
use partial_id::Partial;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use super::request::{Discord, Request, Result};
use super::{
    channel::Channel,
    resource::{Deletable, Patchable, Resource, Snowflake},
    user::PartialUser,
};

#[derive(Debug, Deserialize, Copy, Clone)]
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
    pub typ: MustBe!(1),
    pub components: Vec<ActionRowComponent>,
}

#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ButtonStyle {
    Primary = 1,
    Secondary = 2,
    Success = 3,
    Danger = 4,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionRowComponent {
    #[serde(rename = 2)]
    Button {
        style: ButtonStyle,
        custom_id: Option<String>,
        label: Option<String>,
        #[serde(skip_serializing_if = "std::ops::Not::not", default)]
        disabled: bool,
    },
    #[serde(rename = 3)]
    TextSelectMenu {
        custom_id: String,
        options: Vec<SelectOption>,
        #[serde(skip_serializing_if = "std::ops::Not::not", default)]
        disabled: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelectOption {
    label: String,
    value: String,
    description: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    default: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
}

impl Author {
    pub fn new(name: String) -> Self {
        Self { name }
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
    pub fn new(name: String, value: String) -> Self {
        Self {
            name,
            value,
            inline: false,
        }
    }
    pub fn inlined(name: String, value: String) -> Self {
        Self {
            name,
            value,
            inline: true,
        }
    }
}

#[derive(Serialize)]
struct CreateThread {
    name: String,
}

#[async_trait]
pub trait MessageResource {
    fn endpoint(&self) -> MessageIdentifier;

    fn channel(&self) -> Snowflake<Channel> {
        self.endpoint().channel_id
    }

    fn start_thread_request(&self, name: String) -> Request<Channel> {
        let id = self.endpoint();
        Request::post(
            format!(
                "/channels/{}/messages/{}/threads",
                id.channel_id, id.message_id
            ),
            &CreateThread { name },
        )
    }
    async fn start_thread(&self, client: &Discord, name: String) -> Result<Channel> {
        client.request(self.start_thread_request(name)).await
    }
}

impl MessageResource for MessageIdentifier {
    fn endpoint(&self) -> MessageIdentifier {
        self.clone()
    }
}

impl MessageResource for Message {
    fn endpoint(&self) -> MessageIdentifier {
        self.id
    }
}

impl MessageResource for PartialMessage {
    fn endpoint(&self) -> MessageIdentifier {
        self.id
    }
}

impl<T> Resource<Message> for T
where
    T: MessageResource,
{
    fn uri(&self) -> String {
        let id = self.endpoint();
        format!("/channels/{}/messages/{}", id.channel_id, id.message_id)
    }
}

impl<T> Patchable<Message, PatchMessage> for T where T: MessageResource {}
impl<T> Deletable<Message> for T where T: MessageResource {}
