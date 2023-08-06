use std::fmt::{Display, Formatter};
use std::write;

use partial_id::Partial;
use serde::Deserialize;

use crate::request::Discord;
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

resource! {
    ChannelResource as Snowflake<Channel>;
    use Discord;

    fn get(&self) -> Channel {
        HttpRequest::get(self.endpoint().uri())
    }
    fn send_message(&self, data: CreateMessage) -> Message {
        HttpRequest::post(format!("{}/messages", self.endpoint().uri()), &data)
    }
}

impl ChannelResource for Channel {
    fn endpoint(&self) -> &Snowflake<Channel> {
        &self.id
    }
}

impl ChannelResource for PartialChannel {
    fn endpoint(&self) -> &Snowflake<Channel> {
        &self.id
    }
}
