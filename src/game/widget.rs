use discord::{
    interaction::{MessageComponent, MessageInteraction},
    message::{ActionRow, ActionRowComponent, Button, ButtonStyle, SelectOption, TextSelectMenu},
    resource::Snowflake,
    user::User,
};

use super::{GameMessage, B64_TABLE};

pub struct Event<'a> {
    interaction: Option<&'a MessageInteraction<MessageComponent>>,
}

impl<'a> Event<'a> {
    pub fn none() -> Self {
        Self { interaction: None }
    }
    pub fn component(interaction: &'a MessageInteraction<MessageComponent>) -> Self {
        Self {
            interaction: Some(&interaction),
        }
    }

    pub fn matches<T>(
        &self,
        f: impl FnOnce(&'a MessageInteraction<MessageComponent>) -> Option<T>,
    ) -> Option<T> {
        f(self.interaction?)
    }

    pub fn button(
        &self,
        button: Button,
        mut pressed: impl FnMut(Snowflake<User>),
    ) -> ActionRowComponent {
        if let Button::Action { custom_id, .. } = &button {
            if let Some(i) = self.interaction {
                if i.data.custom_id == custom_id.as_str() {
                    pressed(i.user.id);
                }
            }
        };
        ActionRowComponent::Button(button)
    }
}

impl GameMessage {
    pub fn append_action(
        &mut self,
        action: impl Into<&'static str>,
        style: ButtonStyle,
        name: String,
    ) {
        let button = ActionRowComponent::Button(Button::Action {
            style,
            custom_id: Into::<&'static str>::into(action).into(),
            label: Some(name),
            disabled: false,
        });
        match self.components.last_mut() {
            Some(row) if !row.is_full() => row.components.push(button),
            _ => self.components.push(ActionRow::new(vec![button])),
        }
    }
    pub fn create_select(
        &mut self,
        event: &Event,
        name: String,
        items: impl IntoIterator<Item = String>,
        selected: &mut Vec<usize>,
    ) {
        // get selected values
        let changed = match event.matches(|i| {
            if i.data.custom_id == name {
                Some(&i.data.values)
            } else {
                None
            }
        }) {
            Some(v) => {
                *selected = v
                    .iter()
                    .filter_map(|s| {
                        let first = s.chars().next()?;
                        B64_TABLE.iter().position(|&c| c == first)
                    })
                    .collect();
                true
            }
            _ => false,
        };

        let options: Vec<SelectOption> = items
            .into_iter()
            .enumerate()
            .map(|(i, s)| SelectOption {
                default: selected.contains(&i),
                label: s,
                description: None,
                value: B64_TABLE[i].to_string(),
            })
            .collect();

        if changed {
            selected.retain(|&i| i < options.len());
        }

        // add component
        self.components
            .push(ActionRow::new(vec![ActionRowComponent::TextSelectMenu(
                TextSelectMenu {
                    custom_id: name.clone(),
                    placeholder: Some(name),
                    min_values: 0,
                    max_values: options.len(),
                    options,
                    disabled: false,
                },
            )]));
    }
    pub fn create_number(
        &mut self,
        event: &Event,
        name: String,
        val: &mut i32,
        min: i32,
        max: i32,
    ) {
        // get value
        match event.matches(|i| i.data.custom_id.strip_prefix(&name)) {
            Some("__min") => *val = val.saturating_sub(1).max(min),
            Some("__max") => *val = val.saturating_add(1).min(max),
            _ => (),
        };

        // add components
        self.components.push(ActionRow::new(vec![
            ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Primary,
                custom_id: format!("{}__label", name),
                label: Some(name.clone()),
                disabled: true,
            }),
            ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Primary,
                custom_id: format!("{}__min", name),
                label: Some("<".into()),
                disabled: *val == min,
            }),
            ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Secondary,
                custom_id: format!("{}", name),
                label: Some(val.to_string()),
                disabled: false,
            }),
            ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Primary,
                custom_id: format!("{}__max", name),
                label: Some(">".into()),
                disabled: *val == max,
            }),
        ]));
    }
    pub fn create_join(&mut self, event: &Event, users: &mut Vec<Snowflake<User>>) {
        self.components.push(ActionRow::new(vec![
            event.button(
                Button::Action {
                    style: ButtonStyle::Success,
                    custom_id: "join".into(),
                    label: Some("Join".into()),
                    disabled: false,
                },
                |u| {
                    if !users.contains(&u) {
                        users.push(u);
                    }
                },
            ),
            event.button(
                Button::Action {
                    style: ButtonStyle::Danger,
                    custom_id: "leave".into(),
                    label: Some("Leave".into()),
                    disabled: false,
                },
                |u| {
                    users.retain(|&o| o != u);
                },
            ),
        ]));
    }
    pub fn create_select_grid(
        &mut self,
        event: &Event,
        count: usize,
        selected: &mut Vec<Option<usize>>,
        done: impl FnOnce(&Vec<Option<usize>>) -> bool,
    ) -> bool {
        // TODO: scrolling if too big

        let mut changed = false;

        // for some reason rust-analyzer thinks this is unused
        #[allow(unused_assignments)]
        let mut is_done = false;

        if let Some(i) = event.matches(|i| {
            let s = i.data.custom_id.strip_prefix('#')?;
            let c = s.chars().next()?;
            B64_TABLE
                .iter()
                .position(|&p| p == c)
                .filter(|&i| i < count)
        }) {
            if selected.contains(&Some(i)) {
                // we are not done anymore
                changed = done(selected);
                is_done = false;

                let pos = selected.iter_mut().find(|&&mut s| s == Some(i));
                match pos {
                    Some(s) => *s = None,
                    None => (),
                }

                // remove None values at the end
                while selected.last().is_some_and(|o| o.is_none()) {
                    selected.pop();
                }
            } else {
                let pos = selected.iter_mut().find(|&&mut s| s.is_none());
                match pos {
                    Some(s) => *s = Some(i),
                    None => selected.push(Some(i)),
                }

                // if we are now done it has changed
                changed = done(selected);
                is_done = changed;
            }
        } else {
            is_done = done(selected);
        }

        let mut iter = 0..count;
        loop {
            let mut buttons = Vec::new();
            for _ in 0..5 {
                match iter.next() {
                    Some(i) => {
                        let is_pressed = selected.contains(&Some(i));
                        buttons.push(ActionRowComponent::Button(Button::Action {
                            style: match is_pressed {
                                true => ButtonStyle::Success,
                                false => ButtonStyle::Secondary,
                            },
                            custom_id: format!("#{}", B64_TABLE[i]),
                            label: Some((i + 1).to_string()),
                            disabled: !is_pressed && is_done,
                        }));
                    }
                    None => {
                        if !buttons.is_empty() {
                            self.components.push(ActionRow::new(buttons));
                        }
                        return changed;
                    }
                }
            }
            self.components.push(ActionRow::new(buttons));
        }
    }
}
