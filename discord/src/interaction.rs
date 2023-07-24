use std::mem;

use async_trait::async_trait;
use derive_setters::Setters;
use enumset::{EnumSet, EnumSetType};
use isahc::{
    http::{Method, StatusCode},
    AsyncReadResponseExt,
};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::request::{Client, RequestError};

use super::{
    application::Application,
    channel::Channel,
    command::CommandIdentifier,
    message::{ActionRow, Embed, Message, PatchMessage},
    request::{Request, Result},
    resource::{Deletable, Patchable, Resource, Snowflake},
    user::User,
};

#[derive(Debug)]
pub enum AnyInteraction {
    Command(Interaction<ApplicationCommand>),
    Component(Interaction<MessageComponent>),
}

#[derive(Debug, Deserialize)]
pub struct Interaction<T: 'static> {
    pub data: T,

    #[serde(flatten)]
    pub token: InteractionToken<T>,

    pub channel_id: Snowflake<Channel>,
}

#[derive(Debug, Deserialize)]
pub struct InteractionToken<T: 'static> {
    id: Snowflake<Interaction<T>>,
    token: String,
    application_id: Snowflake<Application>,
}

impl<T: 'static> Drop for InteractionToken<T> {
    fn drop(&mut self) {
        let clone = Self {
            id: self.id,
            token: self.token.clone(),
            application_id: self.application_id,
        };
        tokio::spawn(async move {
            let _ = clone.deferred_update(&Webhook).await;
        });
    }
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

pub struct Webhook;

#[async_trait]
impl Client for Webhook {
    async fn request_weak<T: DeserializeOwned>(
        &self,
        method: Method,
        uri: &str,
        body: Option<&str>,
    ) -> Result<T> {
        // send request
        let http = isahc::Request::builder()
            .method(method)
            .uri("https://discord.com/api/v10".to_owned() + uri);

        let mut response = if let Some(body) = body {
            let request = http
                .header("Content-Type", "application/json")
                .body(body.clone())
                .unwrap();
            println!("{}", request.body());
            isahc::send_async(request)
        } else {
            let request = http.body(()).unwrap();
            isahc::send_async(request)
        }
        .await
        .map_err(|err| {
            if err.is_client() || err.is_server() || err.is_tls() {
                RequestError::Authorization
            } else {
                RequestError::Network
            }
        })?;

        // check errors
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(RequestError::RateLimited);
        }

        let string = response.text().await.unwrap();
        println!("{}", string);

        if response.status().is_client_error() {
            return Err(RequestError::ClientError(response.status()));
        }

        if response.status().is_server_error() {
            return Err(RequestError::ServerError);
        }

        if response.status() == StatusCode::NO_CONTENT {
            Ok(serde_json::from_str("null").unwrap())
        } else {
            serde_json::from_str(&string).map_err(|e| {
                println!("{}", e);
                RequestError::ServerError
            })
        }
    }
}

impl<T> InteractionToken<T> {
    fn uri_response(mut self) -> String {
        let id = self.id;
        let token = mem::replace(&mut self.token, String::new());
        mem::forget(self); // do not run the destructor
        format!("/interactions/{}/{}/callback", id, token)
    }
}

#[async_trait]
pub trait InteractionResource<T: 'static>: Sized {
    fn token(self) -> InteractionToken<T>;

    // TODO: put InteractionResponseIdentifier into Request
    fn reply_request(
        self,
        f: impl FnOnce(CreateReply) -> CreateReply + Send,
    ) -> Request<
        (),
        Webhook,
        InteractionResponseIdentifier,
        impl FnOnce(()) -> InteractionResponseIdentifier + Send,
    > {
        let token = self.token();
        let application_id = token.application_id;
        let str = token.token.clone();

        let reply = f(CreateReply::default());
        Request::post(
            token.uri_response(),
            &Response {
                typ: 4,
                data: reply,
            },
        )
        .map(move |_| InteractionResponseIdentifier {
            application_id,
            token: str,
            message: None,
        })
    }
    async fn reply(
        self,
        client: &Webhook,
        f: impl FnOnce(CreateReply) -> CreateReply + Send,
    ) -> Result<InteractionResponseIdentifier> {
        self.reply_request(f).request(client).await
    }

    // TODO: put these in a ComponentInteractionResource trait
    fn update_request(
        self,
        f: impl FnOnce(CreateReply) -> CreateReply,
    ) -> Request<
        (),
        Webhook,
        InteractionResponseIdentifier,
        impl FnOnce(()) -> InteractionResponseIdentifier + Send,
    > {
        let token = self.token();
        let application_id = token.application_id;
        let str = token.token.clone();

        let reply = f(CreateReply::default());
        Request::post(
            token.uri_response(),
            &Response {
                typ: 7,
                data: reply,
            },
        )
        .map(move |_| InteractionResponseIdentifier {
            application_id,
            token: str,
            message: None,
        })
    }
    async fn update(
        self,
        client: &Webhook,
        f: impl FnOnce(CreateReply) -> CreateReply + Send,
    ) -> Result<InteractionResponseIdentifier> {
        self.update_request(f).request(client).await
    }

    fn deferred_update_request(
        self,
    ) -> Request<
        (),
        Webhook,
        InteractionResponseIdentifier,
        impl FnOnce(()) -> InteractionResponseIdentifier + Send,
    > {
        let token = self.token();
        let application_id = token.application_id;
        let str = token.token.clone();

        Request::post(token.uri_response(), &Response { typ: 7, data: () }).map(move |_| {
            InteractionResponseIdentifier {
                application_id,
                token: str,
                message: None,
            }
        })
    }
    async fn deferred_update(self, client: &Webhook) -> Result<InteractionResponseIdentifier> {
        self.deferred_update_request().request(client).await
    }
}

pub struct InteractionResponseIdentifier {
    application_id: Snowflake<Application>,
    token: String,
    message: Option<Snowflake<Message>>,
}

impl InteractionResponseIdentifier {
    pub fn followup_request(
        &self,
        f: impl FnOnce(CreateReply) -> CreateReply,
    ) -> Request<
        Message,
        Webhook,
        (InteractionResponseIdentifier, Message),
        impl FnOnce(Message) -> (InteractionResponseIdentifier, Message),
    > {
        let reply = f(CreateReply::default());
        let application_id = self.application_id;
        let token = self.token.clone();
        Request::post(
            format!("/webhooks/{}/{}", self.application_id, self.token),
            &reply,
        )
        .map(move |m: Message| {
            (
                InteractionResponseIdentifier {
                    application_id,
                    token,
                    message: Some(m.id.snowflake()),
                },
                m,
            )
        })
    }
    pub async fn followup(
        &self,
        client: &Webhook,
        f: impl FnOnce(CreateReply) -> CreateReply + Send,
    ) -> Result<(InteractionResponseIdentifier, Message)> {
        self.followup_request(f).request(client).await
    }
}

impl Resource<Message> for InteractionResponseIdentifier {
    type Client = Webhook;

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
    fn token(self) -> InteractionToken<T> {
        self
    }
}

impl<T> InteractionResource<T> for Interaction<T> {
    fn token(self) -> InteractionToken<T> {
        self.token
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
