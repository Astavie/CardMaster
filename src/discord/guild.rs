use partial_id::Partial;
use serde::Deserialize;

use crate::discord::resource::{Resource, Snowflake};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Guild {
    pub id: Snowflake<Guild>,
}

pub trait GuildResource {
    fn endpoint(&self) -> Snowflake<Guild>;
}

impl GuildResource for Snowflake<Guild> {
    fn endpoint(&self) -> Snowflake<Guild> {
        self.clone()
    }
}

impl GuildResource for Guild {
    fn endpoint(&self) -> Snowflake<Guild> {
        self.id
    }
}

impl GuildResource for PartialGuild {
    fn endpoint(&self) -> Snowflake<Guild> {
        self.id
    }
}

impl<T> Resource<Guild> for T
where
    T: GuildResource,
{
    fn uri(&self) -> String {
        format!("/guilds/{}", self.endpoint())
    }
}
