use async_trait::async_trait;
use derive_setters::Setters;
use enumset::{EnumSet, EnumSetType};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{
    application::Application,
    channel::Channel,
    command::CommandIdentifier,
    message::{ActionRow, Embed, Message, PatchMessage},
    request::{Discord, Request, Result},
    resource::{Deletable, Patchable, Resource, Snowflake},
    user::User,
};

#[derive(Debug)]
pub enum AnyInteraction {
    Command(Interaction<ApplicationCommand>),
    Component(Interaction<MessageComponent>),
}

#[derive(Debug, Deserialize)]
pub struct Interaction<T> {
    pub data: T,

    #[serde(flatten)]
    pub token: InteractionToken<T>,

    pub channel_id: Snowflake<Channel>,
}

#[derive(Debug, Deserialize)]
pub struct InteractionToken<T> {
    id: Snowflake<Interaction<T>>,
    token: String,
    application_id: Snowflake<Application>,
}

#[derive(Default, Setters, Serialize)]
#[setters(strip_option)]
pub struct CreateReply {
    content: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    embeds: Vec<Embed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    components: Vec<ActionRow>,

    #[serde(skip_serializing_if = "EnumSet::is_empty")]
    flags: EnumSet<ReplyFlag>,
}

#[derive(EnumSetType)]
pub enum ReplyFlag {
    Ephemeral = 6,
    SuppressEmbeds = 2,
}

#[derive(Serialize)]
struct Response<T> {
    #[serde(rename = "type")]
    typ: u8,
    data: T,
}

#[async_trait]
pub trait InteractionResource<T> {
    fn token(&self) -> &InteractionToken<T>;

    fn reply_request(&self, f: impl FnOnce(CreateReply) -> CreateReply) -> Request<()> {
        let reply = f(CreateReply::default());
        let token = self.token();
        Request::post(
            format!("/interactions/{}/{}/callback", token.id, token.token),
            &Response {
                typ: 4,
                data: reply,
            },
        )
    }
    async fn reply(
        &self,
        client: &Discord,
        f: impl FnOnce(CreateReply) -> CreateReply + Send,
    ) -> Result<InteractionResponseIdentifier> {
        client.request(self.reply_request(f)).await?;

        let token = self.token();
        Ok(InteractionResponseIdentifier {
            application_id: token.application_id,
            token: token.token.clone(),
            message: None,
        })
    }

    // TODO: put these in a ComponentInteractionResource trait
    fn update_request(&self, f: impl FnOnce(CreateReply) -> CreateReply) -> Request<()> {
        let reply = f(CreateReply::default());
        let token = self.token();
        Request::post(
            format!("/interactions/{}/{}/callback", token.id, token.token),
            &Response {
                typ: 7,
                data: reply,
            },
        )
    }
    async fn update(
        &self,
        client: &Discord,
        f: impl FnOnce(CreateReply) -> CreateReply + Send,
    ) -> Result<InteractionResponseIdentifier> {
        client.request(self.update_request(f)).await?;

        let token = self.token();
        Ok(InteractionResponseIdentifier {
            application_id: token.application_id,
            token: token.token.clone(),
            message: None,
        })
    }
}

pub struct InteractionResponseIdentifier {
    application_id: Snowflake<Application>,
    token: String,
    message: Option<Snowflake<Message>>,
}

impl Resource<Message> for InteractionResponseIdentifier {
    fn uri(&self) -> String {
        let id = self
            .message
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "@original".to_owned());

        format!(
            "/webhooks/{}/{}/messages/{}",
            self.application_id, self.token, id
        )
    }
}

impl Patchable<Message, PatchMessage> for InteractionResponseIdentifier {}
impl Deletable<Message> for InteractionResponseIdentifier {}

impl<T> InteractionResource<T> for InteractionToken<T> {
    fn token(&self) -> &InteractionToken<T> {
        self
    }
}

impl<T> InteractionResource<T> for Interaction<T> {
    fn token(&self) -> &InteractionToken<T> {
        &self.token
    }
}

impl<'de> Deserialize<'de> for AnyInteraction {
    fn deserialize<D>(d: D) -> ::std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut value = Value::deserialize(d)?;
        let app_id = value.get("application_id").cloned();
        let message = value.get("message").cloned();

        let typ = value.get("type").and_then(Value::as_u64).unwrap();

        let data = value
            .get_mut("data")
            .and_then(Value::as_object_mut)
            .unwrap();

        Ok(match typ {
            2 => {
                data.insert("application_id".to_owned(), app_id.unwrap());
                AnyInteraction::Command(Interaction::deserialize(value).unwrap())
            }
            3 => {
                data.insert("message".to_owned(), message.unwrap().clone());
                AnyInteraction::Component(Interaction::deserialize(value).unwrap())
            }
            _ => panic!("unsupported type {:?}", typ),
        })
    }
}

#[derive(Debug)]
pub enum CommandTarget {
    ChatInput,
    User(Snowflake<User>),
    Message(Snowflake<Message>),
}

impl<'de> Deserialize<'de> for CommandTarget {
    fn deserialize<D>(d: D) -> ::std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(d)?;

        let typ = value.get("type").and_then(Value::as_u64).unwrap();

        Ok(match typ {
            1 => CommandTarget::ChatInput,
            2 => CommandTarget::User(
                Snowflake::deserialize(value.get("target_id").unwrap()).unwrap(),
            ),
            3 => CommandTarget::Message(
                Snowflake::deserialize(value.get("target_id").unwrap()).unwrap(),
            ),
            _ => panic!("unsupported type {:?}", typ),
        })
    }
}

#[derive(Deserialize, Debug)]
pub struct ParamValue {
    pub name: String,
    value: Value,

    #[serde(default)]
    pub options: Vec<ParamValue>,
}

impl ParamValue {
    pub fn as_string(&self) -> Option<&str> {
        self.value.as_str()
    }
    pub fn as_integer(&self) -> Option<i64> {
        self.value.as_i64()
    }
    pub fn as_number(&self) -> Option<f64> {
        self.value.as_f64()
    }
    pub fn as_bool(&self) -> Option<bool> {
        self.value.as_bool()
    }
}

#[derive(Deserialize, Debug)]
pub struct ApplicationCommand {
    #[serde(flatten)]
    pub command: CommandIdentifier,

    pub name: String,

    #[serde(default)]
    pub options: Vec<ParamValue>,

    #[serde(flatten)]
    pub target: CommandTarget,
}

#[derive(Deserialize, Debug)]
pub struct SelectValue {
    pub label: String,
    pub value: String,
    pub description: Option<String>,

    #[serde(default)]
    pub default: bool,
}

#[derive(Deserialize, Debug)]
pub struct MessageComponent {
    pub custom_id: String,
    pub message: Message,

    #[serde(default)]
    pub values: Vec<SelectValue>,
}
