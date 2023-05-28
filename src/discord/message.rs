use async_trait::async_trait;
use derive_setters::Setters;
use partial_id::Partial;
use serde::{Deserialize, Serialize};

use crate::discord::{
    channel::Channel,
    resource::{Deletable, Patchable, Resource, Snowflake},
    user::PartialUser,
};

use super::request::{Client, Request, Result};

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
}

#[derive(Default, Setters, Serialize)]
#[setters(strip_option, borrow_self)]
pub struct CreateMessage {
    content: Option<String>,
}

#[derive(Default, Setters, Serialize)]
#[setters(strip_option, borrow_self)]
pub struct PatchMessage {
    content: Option<String>,
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
    async fn start_thread(&self, client: &impl Client, name: String) -> Result<Channel> {
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
