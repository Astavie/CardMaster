use serde::Deserialize;

use crate::resource::Snowflake;

#[derive(Debug, Deserialize)]
pub struct Interaction {
    id: Snowflake<Interaction>,
}
