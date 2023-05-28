use async_trait::async_trait;
use partial_id::Partial;
use serde::Deserialize;

use crate::discord::{
    message::{CreateMessage, Message},
    request::{Client, Request, Result},
    resource::{Resource, Snowflake},
};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Channel {
    pub id: Snowflake<Channel>,
}

#[async_trait]
pub trait ChannelResource {
    fn endpoint(&self) -> Snowflake<Channel>;

    fn send_message_request(
        &self,
        f: impl for<'a> FnOnce(&'a mut CreateMessage) -> &'a mut CreateMessage,
    ) -> Request<Message> {
        let mut msg = CreateMessage::default();
        f(&mut msg);
        Request::post(format!("/channels/{}/messages", self.endpoint()), &msg)
    }

    async fn send_message(
        &self,
        client: &impl Client,
        f: impl for<'a> FnOnce(&'a mut CreateMessage) -> &'a mut CreateMessage + Send,
    ) -> Result<Message> {
        client.request(self.send_message_request(f)).await
    }
}

impl ChannelResource for Snowflake<Channel> {
    fn endpoint(&self) -> Snowflake<Channel> {
        self.clone()
    }
}

impl ChannelResource for Channel {
    fn endpoint(&self) -> Snowflake<Channel> {
        self.id
    }
}

impl ChannelResource for PartialChannel {
    fn endpoint(&self) -> Snowflake<Channel> {
        self.id
    }
}

impl<T> Resource<Channel> for T
where
    T: ChannelResource,
{
    fn uri(&self) -> String {
        format!("/channels/{}", self.endpoint())
    }
}
