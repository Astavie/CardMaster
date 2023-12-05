use std::{mem, sync::Arc};

use async_trait::async_trait;
use derive_setters::Setters;
use enumset::{EnumSet, EnumSetType};
use isahc::{
    http::{Method, StatusCode},
    AsyncReadResponseExt,
};
use monostate::MustBe;
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use serde_repr::Serialize_repr;

use crate::{
    message::{CreateAttachment, PartialAttachment},
    request::{create_response, Attachments, Client, File, IndexedOr, Request, RequestError},
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
    Component(MessageInteraction<MessageComponent>),

    Modal(Interaction<ModalSubmit>),
    MessageModal(MessageInteraction<ModalSubmit>),
}

#[derive(Debug, Deserialize)]
pub struct MessageInteraction<T: 'static> {
    pub data: T,

    #[serde(flatten)]
    pub token: MessageInteractionToken<T>,
    pub user: User,

    pub channel_id: Snowflake<Channel>,
    pub message: Message,
}

#[derive(Debug, Deserialize)]
pub struct Interaction<T: 'static> {
    pub data: T,

    #[serde(flatten)]
    pub token: InteractionToken<T>,
    pub user: User,

    pub channel_id: Snowflake<Channel>,
}

#[derive(Debug, Deserialize)]
pub struct MessageInteractionToken<T: 'static> {
    id: Snowflake<MessageInteraction<T>>,
    token: String,
    application_id: Snowflake<Application>,
}

#[derive(Debug, Deserialize)]
pub struct InteractionToken<T: 'static> {
    id: Snowflake<Interaction<T>>,
    token: String,
    application_id: Snowflake<Application>,
}

impl<T: 'static> Drop for MessageInteractionToken<T> {
    fn drop(&mut self) {
        // We do nothing to the message
        let clone = MessageInteractionToken {
            id: self.id,
            token: self.token.clone(),
            application_id: self.application_id,
        };
        tokio::spawn(async move {
            let _ = clone.deferred_update(&Webhook).await;
        });
    }
}

impl<T> Drop for InteractionToken<T> {
    fn drop(&mut self) {
        // We let it fail
        // TODO: should this be logged?
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
    attachments: IndexedOr<CreateAttachment, PartialAttachment>,
}

impl Attachments for CreateUpdate {
    fn attachments(&self) -> Vec<Arc<File>> {
        self.attachments.0.iter().map(|a| a.file.clone()).collect()
    }
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

impl<T> Attachments for Response<T>
where
    T: Attachments,
{
    fn attachments(&self) -> Vec<Arc<File>> {
        self.data.attachments()
    }
}

pub struct Webhook;

#[async_trait]
impl Client for Webhook {
    async fn request_weak<T: DeserializeOwned>(
        &self,
        method: Method,
        uri: &str,
        body: Option<&str>,
        files: &[Arc<File>],
    ) -> Result<T> {
        // send request
        let http = isahc::Request::builder()
            .method(method)
            .uri(format!("https://discord.com/api/v10{}", uri));

        let mut response = create_response(http, body, files).await.map_err(|err| {
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

impl<T> InteractionToken<T> {
    fn uri_response(mut self) -> String {
        let id = self.id;
        let token = mem::replace(&mut self.token, String::new());
        mem::forget(self); // do not run the destructor
        format!("/interactions/{}/{}/callback", id.as_int(), token)
    }
}

impl<T> MessageInteractionToken<T> {
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
    type Data: 'static;

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
    #[resource((), client = Webhook)]
    fn modal(self, data: Modal) -> HttpRequest<(), Webhook> {
        let token = self.token();
        HttpRequest::post(token.uri_response(), &Response { typ: 9, data })
    }
}

pub trait MessageInteractionResource: Sized {
    type Data: 'static;

    fn token(self) -> MessageInteractionToken<Self::Data>;

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
    #[resource((), client = Webhook)]
    fn modal(self, data: Modal) -> HttpRequest<(), Webhook> {
        let token = self.token();
        HttpRequest::post(token.uri_response(), &Response { typ: 9, data })
    }

    #[resource(InteractionResponseIdentifier, client = Webhook)]
    fn update(self, data: CreateUpdate) -> ResponseRequest {
        let token = self.token();
        let application_id = token.application_id;
        let str = token.token.clone();

        ResponseRequest(
            HttpRequest::post_attached(token.uri_response(), &Response { typ: 7, data }),
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

impl<T> InteractionResource for InteractionToken<T> {
    type Data = T;
    fn token(self) -> InteractionToken<T> {
        self
    }
}

impl<T> InteractionResource for Interaction<T> {
    type Data = T;
    fn token(self) -> InteractionToken<T> {
        self.token
    }
}

impl<T> MessageInteractionResource for MessageInteractionToken<T> {
    type Data = T;
    fn token(self) -> MessageInteractionToken<T> {
        self
    }
}

impl<T> MessageInteractionResource for MessageInteraction<T> {
    type Data = T;
    fn token(self) -> MessageInteractionToken<T> {
        self.token
    }
}

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

        let typ = value.get("type").and_then(Value::as_u64).unwrap();

        let data = value.get_mut("data").unwrap().as_object_mut().unwrap();

        Ok(match typ {
            2 => {
                data.insert("application_id".into(), app_id.unwrap());
                AnyInteraction::Command(Interaction::deserialize(value).unwrap())
            }
            3 => AnyInteraction::Component(MessageInteraction::deserialize(value).unwrap()),
            5 => {
                if value.get("message").is_some() {
                    AnyInteraction::MessageModal(MessageInteraction::deserialize(value).unwrap())
                } else {
                    AnyInteraction::Modal(Interaction::deserialize(value).unwrap())
                }
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

    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Serialize_repr)]
#[repr(u8)]
pub enum TextStyle {
    Short = 1,
    Paragraph = 2,
}

#[derive(Debug, Serialize)]
pub struct TextActionRow {
    #[serde(rename = "type")]
    typ: MustBe!(1u64),
    pub components: [TextComponent; 1],
}

impl TextActionRow {
    pub fn new(text: TextComponent) -> Self {
        Self {
            typ: monostate::MustBeU64,
            components: [text],
        }
    }
}

fn is_true(b: &bool) -> bool {
    *b
}

#[derive(Debug, Serialize, Setters)]
#[setters(strip_option)]
pub struct TextComponent {
    #[serde(rename = "type")]
    #[setters(skip)]
    typ: MustBe!(4u64),

    pub custom_id: String,
    pub style: TextStyle,
    pub label: String,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    #[serde(skip_serializing_if = "is_true")]
    pub required: bool,
    pub value: Option<String>,
    pub placeholder: Option<String>,
}

impl TextComponent {
    pub fn new<S1, S2>(id: S1, style: TextStyle, label: S2) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Self {
            typ: monostate::MustBeU64,
            custom_id: id.into(),
            style,
            label: label.into(),
            min_length: None,
            max_length: None,
            required: true,
            value: None,
            placeholder: None,
        }
    }
}

impl From<TextComponent> for TextActionRow {
    fn from(value: TextComponent) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Serialize)]
pub struct Modal {
    pub custom_id: String,
    pub title: String,
    pub components: Vec<TextActionRow>,
}

#[derive(Debug, Deserialize)]
pub struct TextValueActionRow {
    #[serde(rename = "type")]
    _typ: MustBe!(1u64),
    pub components: [TextValue; 1],
}

#[derive(Debug, Deserialize)]
pub struct TextValue {
    #[serde(rename = "type")]
    _typ: MustBe!(4u64),
    pub custom_id: String,
    pub value: String,
}

#[derive(Deserialize, Debug)]
pub struct ModalSubmit {
    pub custom_id: String,
    pub components: Vec<TextValueActionRow>,
}
