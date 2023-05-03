use derive_setters::Setters;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{
    application::Application,
    channel::Channel,
    command::CommandIdentifier,
    message::Message,
    request::{Client, Request, Result},
    resource::Snowflake,
    user::User,
};

#[derive(Debug, Deserialize)]
pub struct Interaction {
    #[serde(flatten)]
    pub data: InteractionData,

    #[serde(flatten)]
    pub token: InteractionToken,

    pub user: User,
    pub channel_id: Snowflake<Channel>,
}

#[derive(Debug, Deserialize)]
pub struct InteractionToken {
    id: Snowflake<Interaction>,
    token: String,
    application_id: Snowflake<Application>,
}

#[derive(Default, Setters, Serialize)]
#[setters(strip_option, borrow_self)]
pub struct CreateReply {
    content: Option<String>,
}

#[derive(Serialize)]
struct Response<T> {
    #[serde(rename = "type")]
    typ: u8,
    data: T,
}

pub trait InteractionResource {
    fn token(&self) -> &InteractionToken;

    fn reply_request(&self, f: impl FnOnce(&mut CreateReply) -> &mut CreateReply) -> Request<()> {
        let mut reply = CreateReply::default();
        f(&mut reply);

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
        client: &impl Client,
        f: impl FnOnce(&mut CreateReply) -> &mut CreateReply,
    ) -> Result<()> {
        client.request(&self.reply_request(f)).await
    }
}

impl InteractionResource for InteractionToken {
    fn token(&self) -> &InteractionToken {
        self
    }
}

impl InteractionResource for Interaction {
    fn token(&self) -> &InteractionToken {
        &self.token
    }
}

#[derive(Debug)]
pub enum InteractionData {
    ApplicationCommand(ApplicationCommand),
    MessageComponent(MessageComponent),
}

impl<'de> Deserialize<'de> for InteractionData {
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

                let data = value.get("data").unwrap();
                InteractionData::ApplicationCommand(ApplicationCommand::deserialize(data).unwrap())
            }
            3 => {
                data.insert("message".to_owned(), message.unwrap().clone());

                let data = value.get("data").unwrap();
                InteractionData::MessageComponent(MessageComponent::deserialize(data).unwrap())
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
    pub values: Option<Vec<SelectValue>>,
}
