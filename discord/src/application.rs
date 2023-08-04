use partial_id::Partial;
use serde::Deserialize;

use crate::guild::GuildResource;
use crate::request::{Discord, Request};
use crate::resource::resource;

use super::{command::Commands, resource::Snowflake};

#[derive(Partial)]
#[derive(Debug, Deserialize)]
pub struct Application {
    pub id: Snowflake<Application>,
}

pub trait ApplicationResource {
    fn endpoint(&self) -> &Snowflake<Application>;

    fn global_commands(&self) -> Commands {
        Commands::new(self.endpoint().clone(), None)
    }
    fn guild_commands(&self, guild: &impl GuildResource) -> Commands {
        Commands::new(self.endpoint().clone(), Some(guild.endpoint().clone()))
    }
}

impl ApplicationResource for Snowflake<Application> {
    fn endpoint(&self) -> &Snowflake<Application> {
        self
    }
}
impl ApplicationResource for Application {
    fn endpoint(&self) -> &Snowflake<Application> {
        &self.id
    }
}

pub struct Me;

resource! {
    ApplicationMeResource as Me;
    use Discord;

    fn get(&self) -> Application {
        Request::get("/oauth2/applications/@me")
    }
}
