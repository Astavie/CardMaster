use async_trait::async_trait;

use crate::{
    discord::{
        interaction::{CreateReply, Interaction, MessageComponent},
        request::{Discord, Result},
    },
    game::{Game, GameUI, Logic},
};

pub struct CAH {}

#[async_trait]
impl Logic<()> for CAH {
    async fn logic(
        &mut self,
        client: &Discord,
        ui: &mut GameUI,
        i: Interaction<MessageComponent>,
    ) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl Game for CAH {
    const NAME: &'static str = "Cards Against Humanity";

    fn new() -> Self {
        CAH {}
    }

    fn lobby_msg_reply<'a>(msg: &'a mut CreateReply) -> &'a mut CreateReply {
        msg.content("# Crappy Ableist Humor".to_owned())
    }
}
