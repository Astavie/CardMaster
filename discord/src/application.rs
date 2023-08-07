use partial_id::Partial;
use serde::Deserialize;

use crate::guild::GuildResource;
use crate::request::HttpRequest;
use crate::resource::{resource, Endpoint};

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

impl Endpoint for Snowflake<Application> {
    fn uri(&self) -> String {
        // you can only ever get your own application data
        "/applications/@me".into()
    }
}

pub struct Me;

impl Me {
    #[resource(Application)]
    pub fn get(&self) -> HttpRequest<Application> {
        HttpRequest::get("/applications/@me")
    }
}
