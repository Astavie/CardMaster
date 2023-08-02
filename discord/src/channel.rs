use std::fmt::{Display, Formatter};
use std::write;

use async_trait::async_trait;
use partial_id::Partial;
use serde::Deserialize;

use crate::resource::Endpoint;

use super::request::Discord;
use super::{
    message::{CreateMessage, Message},
    request::{Request, Result},
    resource::{Resource, Snowflake},
};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Channel {
    pub id: Snowflake<Channel>,
}

impl Display for Snowflake<Channel> {
    fn fmt(&self, f: &mut Formatter<'_>) -> ::std::fmt::Result {
        write!(f, "<#{}>", self.as_int())
    }
}

impl Endpoint for Snowflake<Channel> {
    type Result = Channel;
    fn uri(&self) -> String {
        format!("/channels/{}", self.as_int())
    }
}

#[async_trait]
pub trait ChannelResource: Resource<Endpoint = Snowflake<Channel>> {
    fn send_message_request(
        &self,
        f: impl FnOnce(CreateMessage) -> CreateMessage,
    ) -> Request<Message> {
        let msg = f(CreateMessage::default());
        Request::post(format!("{}/messages", self.endpoint().uri()), &msg)
    }
    async fn send_message(
        &self,
        client: &Discord,
        f: impl FnOnce(CreateMessage) -> CreateMessage + Send,
    ) -> Result<Message> {
        self.send_message_request(f).request(client).await
    }
}

impl<T> ChannelResource for T where T: Resource<Endpoint = Snowflake<Channel>> {}

impl Resource for Channel {
    type Endpoint = Snowflake<Channel>;
    fn endpoint(&self) -> &Self::Endpoint {
        &self.id
    }
}
impl Resource for PartialChannel {
    type Endpoint = Snowflake<Channel>;
    fn endpoint(&self) -> &Self::Endpoint {
        &self.id
    }
}
