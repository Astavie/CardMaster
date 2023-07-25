use partial_id::Partial;
use serde::Deserialize;

use crate::guild::Guild;
use crate::resource::Resource;

use super::request::Discord;
use super::{
    command::Commands,
    request::{Request, Result},
    resource::Snowflake,
};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Application {
    pub id: Snowflake<Application>,
}

pub trait ApplicationResource {
    fn endpoint(&self) -> Snowflake<Application>;

    fn global_commands(&self) -> Commands {
        Commands::new(self.endpoint(), None)
    }
    fn guild_commands(&self, guild: &impl Resource<Endpoint = Snowflake<Guild>>) -> Commands {
        Commands::new(self.endpoint(), Some(guild.endpoint().clone()))
    }
}

impl Application {
    pub fn get_request() -> Request<Self> {
        Request::get("/oauth2/applications/@me".to_owned())
    }
    pub async fn get(client: &Discord) -> Result<Self> {
        Self::get_request().request(client).await
    }
}

impl ApplicationResource for Snowflake<Application> {
    fn endpoint(&self) -> Snowflake<Application> {
        self.clone()
    }
}
impl ApplicationResource for Application {
    fn endpoint(&self) -> Snowflake<Application> {
        self.id
    }
}
