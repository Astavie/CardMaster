use derive_setters::Setters;
use partial_id::Partial;
use serde::{Deserialize, Serialize};

use crate::{
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
    pub(crate) fn new(channel_id: Snowflake<Channel>, message_id: u64) -> Self {
        Self {
            channel_id,
            message_id: Snowflake::new(message_id),
        }
    }
}

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(flatten)]
    id: MessageIdentifier,

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

pub trait MessageResource {
    fn endpoint(&self) -> MessageIdentifier;

    fn channel(&self) -> Snowflake<Channel> {
        self.endpoint().channel_id
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
