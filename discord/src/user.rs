use async_trait::async_trait;
use derive_setters::Setters;
use partial_id::Partial;
use serde::{Deserialize, Serialize};

use crate::resource::Endpoint;

use super::request::Discord;
use super::{
    channel::{Channel, ChannelResource},
    message::{CreateMessage, Message},
    request::{Request, Result},
    resource::{Resource, Snowflake},
};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct User {
    pub id: Snowflake<User>,
    pub username: String,
}

#[derive(Default, Setters, Serialize)]
#[setters(strip_option)]
pub struct PatchUser {
    username: Option<String>,
}

#[derive(Serialize)]
struct DMRequest {
    recipient_id: Snowflake<User>,
}

#[async_trait]
pub trait UserResource: Resource<Endpoint = Snowflake<User>> {
    fn create_dm_request(&self) -> Request<Channel> {
        Request::post(
            "/users/@me/channels".to_owned(),
            &DMRequest {
                recipient_id: self.endpoint().clone(),
            },
        )
    }

    async fn create_dm(&self, client: &Discord) -> Result<Channel> {
        self.create_dm_request().request(client).await
    }

    async fn send_message(
        &self,
        client: &Discord,
        f: impl FnOnce(CreateMessage) -> CreateMessage + Send,
    ) -> Result<Message> {
        let channel = self.create_dm(client).await?;
        channel.send_message(client, f).await
    }
}

impl<T> UserResource for T where T: Resource<Endpoint = Snowflake<User>> {}

impl Endpoint for Snowflake<User> {
    type Result = User;
    fn uri(&self) -> String {
        format!("/users/{}", self)
    }
}

impl Resource for User {
    type Endpoint = Snowflake<User>;
    fn endpoint(&self) -> &Self::Endpoint {
        &self.id
    }
}
impl Resource for PartialUser {
    type Endpoint = Snowflake<User>;
    fn endpoint(&self) -> &Self::Endpoint {
        &self.id
    }
}

#[derive(Clone, Copy)]
pub struct Me;

impl Endpoint for Me {
    type Result = User;
    type Patch = PatchUser;
    fn uri(&self) -> String {
        "/users/@me".into()
    }
}
