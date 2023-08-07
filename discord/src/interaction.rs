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

use crate::{
    request::{Client, Request, RequestError},
    resource::{resource, Endpoint},
};

use super::{
    application::Application,
    channel::Channel,
    command::CommandIdentifier,
    message::{ActionRow, Embed, Message, PatchMessage},
    request::{HttpRequest, Result},
    resource::Snowflake,
    user::User,
};

#[derive(Debug)]
pub enum AnyInteraction {
    Command(Interaction<ApplicationCommand>),
    Component(Interaction<MessageComponent>),
}

#[derive(Debug, Deserialize)]
pub struct Interaction<T: 'static + DropToken> {
    pub data: T,

    #[serde(flatten)]
    pub token: InteractionToken<T>,
    pub user: User,

    pub channel_id: Snowflake<Channel>,
}

#[derive(Debug, Deserialize)]
pub struct InteractionToken<T: 'static + DropToken> {
    id: Snowflake<Interaction<T>>,
    token: String,
    application_id: Snowflake<Application>,
}

impl<T: 'static + DropToken> Drop for InteractionToken<T> {
    fn drop(&mut self) {
        T::drop(self);
    }
}

pub trait DropToken: Sized {
    fn drop(t: &mut InteractionToken<Self>);
}

impl DropToken for ApplicationCommand {
    fn drop(_t: &mut InteractionToken<Self>) {
        // We let it fail
        // TODO: should this be logged?
    }
}

impl DropToken for MessageComponent {
    fn drop(t: &mut InteractionToken<Self>) {
        // We do nothing to the message
        let clone = InteractionToken {
            id: t.id,
            token: t.token.clone(),
            application_id: t.application_id,
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

#[derive(Default, Setters, Serialize)]
#[setters(strip_option)]
pub struct CreateUpdate {
    content: Option<String>,

    // send these even if empty, so they can also be removed
    embeds: Vec<Embed>,
    components: Vec<ActionRow>,
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
            .uri(format!("https://discord.com/api/v10{}", uri));

        let mut response = if let Some(body) = body {
            let request = http
                .header("Content-Type", "application/json")
                .body(body.clone())
                .unwrap();
            // println!("{}", request.body());
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
        // println!("{}", string);

        if response.status().is_client_error() {
            return Err(RequestError::ClientError(response.status()));
        }

        if response.status().is_server_error() {
            return Err(RequestError::ServerError);
        }

        if response.status() == StatusCode::NO_CONTENT {
            serde_json::from_str("null")
        } else {
            serde_json::from_str(&string)
        }
        .map_err(|e| {
            println!("{}", e);
            RequestError::ServerError
        })
    }
}

impl<T: DropToken> InteractionToken<T> {
    fn uri_response(mut self) -> String {
        let id = self.id;
        let token = mem::replace(&mut self.token, String::new());
        mem::forget(self); // do not run the destructor
        format!("/interactions/{}/{}/callback", id.as_int(), token)
    }
}

pub struct ResponseRequest(HttpRequest<(), Webhook>, InteractionResponseIdentifier);
pub struct MessageResponseRequest(HttpRequest<Message, Webhook>, InteractionResponseIdentifier);

#[async_trait]
impl Request<Webhook> for ResponseRequest {
    type Output = InteractionResponseIdentifier;

    async fn request_weak(self, client: &Webhook) -> Result<Self::Output> {
        self.0.request_weak(client).await?;
        Ok(self.1)
    }
    async fn request(self, client: &Webhook) -> Result<Self::Output> {
        self.0.request(client).await?;
        Ok(self.1)
    }
}

#[async_trait]
impl Request<Webhook> for MessageResponseRequest {
    type Output = (InteractionResponseIdentifier, Message);

    async fn request_weak(mut self, client: &Webhook) -> Result<Self::Output> {
        let m = self.0.request_weak(client).await?;
        self.1.message = Some(m.id.snowflake());
        Ok((self.1, m))
    }
    async fn request(mut self, client: &Webhook) -> Result<Self::Output> {
        let m = self.0.request(client).await?;
        self.1.message = Some(m.id.snowflake());
        Ok((self.1, m))
    }
}

pub trait InteractionResource: Sized {
    type Data: 'static + DropToken;

    fn token(self) -> InteractionToken<Self::Data>;

    fn forget(self) {
        let mut token = self.token();
        let _ = mem::replace(&mut token.token, String::new());
        mem::forget(token); // do not run the destructor
    }

    #[resource(InteractionResponseIdentifier, client = Webhook)]
    fn reply(self, data: CreateReply) -> ResponseRequest {
        let token = self.token();
        let application_id = token.application_id;
        let str = token.token.clone();

        ResponseRequest(
            HttpRequest::post(token.uri_response(), &Response { typ: 4, data }),
            InteractionResponseIdentifier {
                application_id,
                token: str,
                message: None,
            },
        )
    }
}

pub trait ComponentInteractionResource: InteractionResource<Data = MessageComponent> {
    #[resource(InteractionResponseIdentifier, client = Webhook)]
    fn update(self, data: CreateUpdate) -> ResponseRequest {
        let token = self.token();
        let application_id = token.application_id;
        let str = token.token.clone();

        ResponseRequest(
            HttpRequest::post(token.uri_response(), &Response { typ: 7, data }),
            InteractionResponseIdentifier {
                application_id,
                token: str,
                message: None,
            },
        )
    }
    #[resource(InteractionResponseIdentifier, client = Webhook)]
    fn deferred_update(self) -> ResponseRequest {
        let token = self.token();
        let application_id = token.application_id;
        let str = token.token.clone();

        ResponseRequest(
            HttpRequest::post(token.uri_response(), &Response { typ: 7, data: () }),
            InteractionResponseIdentifier {
                application_id,
                token: str,
                message: None,
            },
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct InteractionResponseIdentifier {
    application_id: Snowflake<Application>,
    token: String,
    message: Option<Snowflake<Message>>,
}

impl InteractionResponseIdentifier {
    #[resource(Message, client = Webhook)]
    pub fn get(&self) -> HttpRequest<Message, Webhook> {
        HttpRequest::get(self.uri())
    }
    #[resource(Message, client = Webhook)]
    pub fn patch(&self, data: PatchMessage) -> HttpRequest<Message, Webhook> {
        HttpRequest::patch(self.uri(), &data)
    }
    #[resource(Message, client = Webhook)]
    pub fn delete(self) -> HttpRequest<Message, Webhook> {
        HttpRequest::delete(self.uri())
    }

    #[resource((InteractionResponseIdentifier, Message), client = Webhook)]
    pub fn followup(&self, data: CreateReply) -> MessageResponseRequest {
        let application_id = self.application_id;
        let token = self.token.clone();

        MessageResponseRequest(
            HttpRequest::post(
                format!("/webhooks/{}/{}", application_id.as_int(), token),
                &data,
            ),
            InteractionResponseIdentifier {
                application_id,
                token,
                message: None,
            },
        )
    }
}

impl Endpoint for InteractionResponseIdentifier {
    fn uri(&self) -> String {
        let id = self
            .message
            .as_ref()
            .map(|id| id.as_int().to_string())
            .unwrap_or_else(|| "@original".into());

        format!(
            "/webhooks/{}/{}/messages/{}",
            self.application_id.as_int(),
            self.token,
            id
        )
    }
}

impl<T: DropToken> InteractionResource for InteractionToken<T> {
    type Data = T;
    fn token(self) -> InteractionToken<T> {
        self
    }
}

impl<T: DropToken> InteractionResource for Interaction<T> {
    type Data = T;
    fn token(self) -> InteractionToken<T> {
        self.token
    }
}

impl<T> ComponentInteractionResource for T where T: InteractionResource<Data = MessageComponent> {}

impl<'de> Deserialize<'de> for AnyInteraction {
    fn deserialize<D>(d: D) -> ::std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut value = Value::deserialize(d)?;

        // make sure "user" exists
        if !value.get("user").is_some() {
            let user = value.get("member").unwrap().get("user").unwrap().clone();
            value.as_object_mut().unwrap().insert("user".into(), user);
        }

        let app_id = value.get("application_id").cloned();
        let message = value.get("message").cloned();

        let typ = value.get("type").and_then(Value::as_u64).unwrap();

        let data = value.get_mut("data").unwrap().as_object_mut().unwrap();

        Ok(match typ {
            2 => {
                data.insert("application_id".into(), app_id.unwrap());
                AnyInteraction::Command(Interaction::deserialize(value).unwrap())
            }
            3 => {
                data.insert("message".into(), message.unwrap().clone());
                AnyInteraction::Component(Interaction::deserialize(value).unwrap())
            }
            _ => panic!("unsupported type {:?}", typ),
        })
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "target_id")]
pub enum CommandTarget {
    #[serde(rename = 1)]
    ChatInput,
    #[serde(rename = 2)]
    User(Snowflake<User>),
    #[serde(rename = 3)]
    Message(Snowflake<Message>),
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
pub struct MessageComponent {
    pub custom_id: String,
    pub message: Message,

    #[serde(default)]
    pub values: Vec<String>,
}
