use partial_id::Partial;
use serde::Deserialize;

use crate::resource::Endpoint;

use super::resource::{Resource, Snowflake};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Guild {
    pub id: Snowflake<Guild>,
    pub name: String,
}

impl Endpoint for Snowflake<Guild> {
    type Result = Guild;
    fn uri(&self) -> String {
        format!("/guilds/{}", self.as_int())
    }
}

impl Resource for Guild {
    type Endpoint = Snowflake<Guild>;
    fn endpoint(&self) -> &Self::Endpoint {
        &self.id
    }
}
impl Resource for PartialGuild {
    type Endpoint = Snowflake<Guild>;
    fn endpoint(&self) -> &Self::Endpoint {
        &self.id
    }
}
