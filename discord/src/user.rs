use std::fmt::{Display, Formatter};

use derive_setters::Setters;
use partial_id::Partial;
use serde::{Deserialize, Serialize};

use crate::guild::PartialGuild;
use crate::resource::resource;

use super::request::Discord;
use super::{channel::Channel, request::Request, resource::Snowflake};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct User {
    pub id: Snowflake<User>,
    pub username: String,
}

impl Display for Snowflake<User> {
    fn fmt(&self, f: &mut Formatter<'_>) -> ::std::fmt::Result {
        f.write_fmt(format_args!("<@{}>", self.as_int()))
    }
}

impl Snowflake<User> {
    pub fn uri(&self) -> String {
        format!("/users/{}", self.as_int())
    }
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

resource! {
    UserResource as Snowflake<User>;
    use Discord;

    fn get(&self) -> User {
        Request::get(self.endpoint().uri())
    }

    fn create_dm(&self) -> Channel {
        Request::post(
            "/users/@me/channels",
            &DMRequest {
                recipient_id: self.endpoint().clone(),
            },
        )
    }
}

impl UserResource for User {
    fn endpoint(&self) -> &Snowflake<User> {
        &self.id
    }
}
impl UserResource for PartialUser {
    fn endpoint(&self) -> &Snowflake<User> {
        &self.id
    }
}

pub struct Me;

resource! {
    MeResource as Me;
    use Discord;

    fn get(&self) -> User {
        Request::get("/users/@me")
    }
    fn patch(&self, data: PatchUser) -> User {
        Request::patch("/users/@me", &data)
    }

    fn get_guilds(&self) -> Vec<PartialGuild> {
        Request::get("/users/@me/guilds")
    }
}
