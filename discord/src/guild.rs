use partial_id::Partial;
use serde::Deserialize;

use crate::request::Discord;
use crate::request::HttpRequest;
use crate::resource::resource;
use crate::resource::Endpoint;

use super::resource::Snowflake;

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Guild {
    pub id: Snowflake<Guild>,
    pub name: String,
}

impl Endpoint for Snowflake<Guild> {
    fn uri(&self) -> String {
        format!("/guilds/{}", self.as_int())
    }
}

resource! {
    GuildResource as Snowflake<Guild>;
    use Discord;

    fn get(&self) -> Guild {
        HttpRequest::get(self.endpoint().uri())
    }
}

impl GuildResource for Guild {
    fn endpoint(&self) -> &Snowflake<Guild> {
        &self.id
    }
}

impl GuildResource for PartialGuild {
    fn endpoint(&self) -> &Snowflake<Guild> {
        &self.id
    }
}
