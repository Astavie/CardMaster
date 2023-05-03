use partial_id::Partial;
use serde::Deserialize;

use crate::{
    command::Commands,
    guild::GuildResource,
    request::{Client, Request, Result},
    resource::Snowflake,
};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Application {
    id: Snowflake<Application>,
}

impl Application {
    pub fn get_request() -> Request<Self> {
        Request::get("/oauth2/applications/@me".to_owned())
    }
    pub async fn get(client: &impl Client) -> Result<Self> {
        client.request(Self::get_request()).await
    }

    pub fn global_commands(&self) -> Commands {
        Commands::new(self.id, None)
    }
    pub fn guild_commands(&self, guild: &impl GuildResource) -> Commands {
        Commands::new(self.id, Some(guild.endpoint()))
    }
}
