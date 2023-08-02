use async_trait::async_trait;
use discord::interaction::{Interaction, MessageComponent};

use crate::game::{Flow, GameUI, Logic};

use super::Data;

pub struct Read {
    pub data: Data,
}

#[async_trait]
impl Logic for Read {
    type Return = ();
    async fn logic(&mut self, ui: &mut GameUI, i: Interaction<MessageComponent>) -> Flow<()> {
        // TODO
        Flow::Continue
    }
}
