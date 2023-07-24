use async_trait::async_trait;
use derive_setters::Setters;
use partial_id::Partial;
use serde::{Deserialize, Serialize};

use super::request::Discord;
use super::{
    channel::{Channel, ChannelResource},
    message::{CreateMessage, Message},
    request::{Request, Result},
    resource::{Patchable, Resource, Snowflake},
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
pub trait UserResource {
    fn endpoint(&self) -> Snowflake<User>;

    fn create_dm_request(&self) -> Request<Channel> {
        Request::post(
            "/users/@me/channels".to_owned(),
            &DMRequest {
                recipient_id: self.endpoint(),
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

impl UserResource for Snowflake<User> {
    fn endpoint(&self) -> Snowflake<User> {
        self.clone()
    }
}

impl UserResource for User {
    fn endpoint(&self) -> Snowflake<User> {
        self.id
    }
}

impl UserResource for PartialUser {
    fn endpoint(&self) -> Snowflake<User> {
        self.id
    }
}

impl<T> Resource<User> for T
where
    T: UserResource,
{
    fn uri(&self) -> String {
        format!("/users/{}", self.endpoint())
    }
}

pub struct Me;

impl Resource<User> for Me {
    fn uri(&self) -> String {
        "/users/@me".into()
    }
}

impl Patchable<User, PatchUser> for Me {}
