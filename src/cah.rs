use async_trait::async_trait;

use crate::game::{Flow, Game, GameMessage, GameUI, Logic, Setup, SetupOption};

use discord::interaction::{Interaction, MessageComponent};

pub struct CAH {
    setup: Setup,
}

#[async_trait]
impl Logic<()> for CAH {
    async fn logic(&mut self, ui: &mut GameUI, i: Interaction<MessageComponent>) -> Flow<()> {
        self.setup.logic(ui, i).await?;
        Flow::Return(())
    }
}

#[async_trait]
impl Game for CAH {
    const NAME: &'static str = "Crappy Ableist Humor";
    const COLOR: u32 = 0x000000;

    fn new() -> Self {
        CAH {
            setup: Setup {
                options: vec![
                    ("Cards".to_owned(), SetupOption::Number(5, 20, 10)),
                    ("Points".to_owned(), SetupOption::Number(1, i32::MAX, 8)),
                ],
            },
        }
    }

    fn lobby_msg_reply(&self) -> GameMessage {
        Self::message(Vec::new(), self.setup.render())
    }
}
