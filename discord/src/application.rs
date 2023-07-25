use partial_id::Partial;
use serde::Deserialize;

use crate::guild::Guild;
use crate::resource::{Endpoint, Resource};

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
    fn guild_commands(&self, guild: &impl Resource<Endpoint = Snowflake<Guild>>) -> Commands {
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

impl Endpoint for Me {
    type Result = Application;
    fn uri(&self) -> String {
        "/oauth2/applications/@me".into()
    }
}
