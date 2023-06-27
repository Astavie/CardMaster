use partial_id::Partial;
use serde::Deserialize;

use super::request::Discord;
use super::{
    command::Commands,
    guild::GuildResource,
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
    fn guild_commands(&self, guild: &impl GuildResource) -> Commands {
        Commands::new(self.endpoint(), Some(guild.endpoint()))
    }
}

impl Application {
    pub fn get_request() -> Request<Self> {
        Request::get("/oauth2/applications/@me".to_owned())
    }
    pub async fn get(client: &Discord) -> Result<Self> {
        client.request(Self::get_request()).await
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
