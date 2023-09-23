use std::fmt::{Display, Formatter};
use std::write;

use partial_id::Partial;
use serde::Deserialize;

use crate::resource::{resource, Endpoint};

use super::{
    message::{CreateMessage, Message},
    request::HttpRequest,
    resource::Snowflake,
};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Channel {
    pub id: Snowflake<Channel>,
    pub name: Option<String>,
}

impl Display for Snowflake<Channel> {
    fn fmt(&self, f: &mut Formatter<'_>) -> ::std::fmt::Result {
        write!(f, "<#{}>", self.as_int())
    }
}

impl Endpoint for Snowflake<Channel> {
    fn uri(&self) -> String {
        format!("/channels/{}", self.as_int())
    }
}

pub trait ChannelResource {
    fn endpoint(&self) -> Snowflake<Channel>;

    #[resource(Channel)]
    fn get(&self) -> HttpRequest<Channel> {
        HttpRequest::get(self.endpoint().uri())
    }
    #[resource(Message)]
    fn send_message(&self, data: CreateMessage) -> HttpRequest<Message> {
        HttpRequest::post_attached(format!("{}/messages", self.endpoint().uri()), &data)
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
