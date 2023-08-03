use discord::{
    interaction::{Interaction, MessageComponent},
    message::{ActionRow, ActionRowComponent, Button, ButtonStyle},
};
use monostate::MustBeU64;

use super::{Flow, Menu, B64_TABLE};

pub struct SelectionGrid {
    pub count: usize,
    pub selected: Vec<Option<usize>>,
    pub disable_unselected: bool,
}

impl Menu for SelectionGrid {
    type Update = usize;

    fn render(&self) -> Vec<ActionRow> {
        // TODO: scrolling if too big

        let mut iter = 0..self.count;

        let mut rows = Vec::new();
        loop {
            let mut buttons = Vec::new();
            for _ in 0..5 {
                match iter.next() {
                    Some(i) => {
                        let selected = self.selected.contains(&Some(i));
                        buttons.push(ActionRowComponent::Button(Button::Action {
                            style: match selected {
                                true => ButtonStyle::Success,
                                false => ButtonStyle::Secondary,
                            },
                            custom_id: format!("#{}", B64_TABLE[i]),
                            label: Some((i + 1).to_string()),
                            disabled: !selected && self.disable_unselected,
                        }))
                    }
                    None => {
                        if !buttons.is_empty() {
                            rows.push(ActionRow {
                                typ: MustBeU64::<1>,
                                components: buttons,
                            });
                        }
                        return rows;
                    }
                }
            }
            rows.push(ActionRow {
                typ: MustBeU64::<1>,
                components: buttons,
            });
        }
    }

    fn update(&mut self, it: &Interaction<MessageComponent>) -> Flow<usize> {
        let mut chars = it.data.custom_id.chars();
        if chars.next()? != '#' {
            None?;
        }

        let next = chars.next()?;
        let pos = B64_TABLE.iter().position(|&c| c == next)?;
        Flow::Return(pos)
    }
}

pub struct ChoiceGrid {
    pub shuffle: Vec<usize>,
}

impl Menu for ChoiceGrid {
    type Update = usize;

    fn render(&self) -> Vec<ActionRow> {
        // TODO: scrolling if too big

        let mut iter = self.shuffle.iter().copied().enumerate();

        let mut rows = Vec::new();
        loop {
            let mut buttons = Vec::new();
            for _ in 0..5 {
                match iter.next() {
                    Some((n, index)) => buttons.push(ActionRowComponent::Button(Button::Action {
                        style: ButtonStyle::Primary,
                        custom_id: format!("#{}", B64_TABLE[index]),
                        label: Some((n + 1).to_string()),
                        disabled: false,
                    })),
                    None => {
                        if !buttons.is_empty() {
                            rows.push(ActionRow {
                                typ: MustBeU64::<1>,
                                components: buttons,
                            });
                        }
                        return rows;
                    }
                }
            }
            rows.push(ActionRow {
                typ: MustBeU64::<1>,
                components: buttons,
            });
        }
    }

    fn update(&mut self, it: &Interaction<MessageComponent>) -> Flow<usize> {
        let mut chars = it.data.custom_id.chars();
        if chars.next()? != '#' {
            None?;
        }

        let next = chars.next()?;
        let pos = B64_TABLE.iter().position(|&c| c == next)?;
        Flow::Return(pos)
    }
}
