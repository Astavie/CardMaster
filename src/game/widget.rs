use derive_setters::Setters;
use discord::{
    interaction::{Interaction, MessageComponent},
    message::{ActionRow, ActionRowComponent, Button, ButtonStyle, SelectOption, TextSelectMenu},
    resource::Snowflake,
    user::User,
};

use super::{Flow, GameMessage, Menu, B64_TABLE};

pub struct Event<'a> {
    interaction: Option<&'a Interaction<MessageComponent>>,
}

impl<'a> Event<'a> {
    pub fn none() -> Self {
        Self { interaction: None }
    }
    pub fn component(interaction: &'a Interaction<MessageComponent>) -> Self {
        Self {
            interaction: Some(interaction),
        }
    }

    pub fn custom_id(&self) -> Option<&str> {
        match self.interaction {
            Some(i) => Some(&i.data.custom_id),
            None => None,
        }
    }
    pub fn values(&self, id: &str) -> Option<&Vec<String>> {
        match self.interaction {
            Some(i) if i.data.custom_id == id => Some(&i.data.values),
            _ => None,
        }
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

pub trait Widget: Sized {
    fn create(self, msg: &mut GameMessage, event: &Event) -> Flow<bool>;

    fn render(self, event: Event) -> Flow<(GameMessage, bool)> {
        let mut msg = GameMessage {
            fields: Vec::new(),
            components: Vec::new(),
        };
        match self.create(&mut msg, &event) {
            Flow::Return(t) => Flow::Return((msg, t)),
            Flow::Continue => Flow::Continue,
            Flow::Exit => Flow::Exit,
        }
    }
}

#[derive(Setters)]
#[setters(strip_option)]
pub struct MultiSelect<'a, I>
where
    I: IntoIterator<Item = String>,
{
    #[setters(skip)]
    id: String,
    #[setters(skip)]
    items: I,
    #[setters(skip)]
    selected: &'a mut Vec<usize>,

    name: Option<String>,
}

impl<'a, I> MultiSelect<'a, I>
where
    I: IntoIterator<Item = String>,
{
    pub fn new<S: Into<String>>(id: S, items: I, selected: &'a mut Vec<usize>) -> Self {
        Self {
            id: id.into(),
            name: None,
            items,
            selected,
        }
    }
}

impl<'a, I> Widget for MultiSelect<'a, I>
where
    I: IntoIterator<Item = String>,
{
    fn create(self, msg: &mut GameMessage, event: &Event) -> Flow<bool> {
        // get selected values
        let changed = match event.values(&self.id) {
            Some(v) => {
                *self.selected = v
                    .iter()
                    .filter_map(|s| {
                        let first = s.chars().next().unwrap();
                        B64_TABLE.iter().position(|&c| c == first)
                    })
                    .collect();
                true
            }
            _ => false,
        };

        let options: Vec<SelectOption> = self
            .items
            .into_iter()
            .enumerate()
            .map(|(i, s)| SelectOption {
                default: self.selected.contains(&i),
                label: s,
                description: None,
                value: B64_TABLE[i].to_string(),
            })
            .collect();

        if changed {
            self.selected.retain(|&i| i < options.len());
        }

        // add component
        msg.components
            .push(ActionRow::new(vec![ActionRowComponent::TextSelectMenu(
                TextSelectMenu {
                    custom_id: self.id,
                    placeholder: self.name,
                    min_values: 0,
                    max_values: options.len(),
                    options,
                    disabled: false,
                },
            )]));

        // return new state
        Flow::Return(changed)
    }
}

#[derive(Setters)]
#[setters(strip_option)]
pub struct NumberSelect<'a> {
    #[setters(skip)]
    id: String,
    #[setters(skip)]
    val: &'a mut i32,

    min: i32,
    max: i32,
    name: Option<String>,
}

impl<'a> NumberSelect<'a> {
    pub fn new<S: Into<String>>(id: S, val: &'a mut i32) -> Self {
        Self {
            id: id.into(),
            val,
            min: 0,
            max: i32::MAX,
            name: None,
        }
    }
}

impl<'a> Widget for NumberSelect<'a> {
    fn create(self, msg: &mut GameMessage, event: &Event) -> Flow<bool> {
        // get value
        let changed = match event.custom_id().and_then(|s| s.strip_prefix(&self.id)) {
            Some("__min") => {
                *self.val = self.val.saturating_sub(1).max(self.min);
                true
            }
            Some("__max") => {
                *self.val = self.val.saturating_add(1).min(self.max);
                true
            }
            _ => false,
        };

        // add components
        let mut row = ActionRow::new(Vec::new());
        if let Some(name) = self.name {
            row.components
                .push(ActionRowComponent::Button(Button::Action {
                    style: ButtonStyle::Primary,
                    custom_id: format!("{}__label", self.id),
                    label: Some(name),
                    disabled: true,
                }));
        }
        row.components
            .push(ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Primary,
                custom_id: format!("{}__min", self.id),
                label: Some("<".into()),
                disabled: *self.val == self.min,
            }));
        row.components
            .push(ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Secondary,
                custom_id: format!("{}", self.id),
                label: Some(self.val.to_string()),
                disabled: false,
            }));
        row.components
            .push(ActionRowComponent::Button(Button::Action {
                style: ButtonStyle::Primary,
                custom_id: format!("{}__max", self.id),
                label: Some(">".into()),
                disabled: *self.val == self.max,
            }));

        msg.components.push(row);

        // return new state
        Flow::Return(changed)
    }
}

pub struct JoinButtons<'a>(pub &'a mut Vec<Snowflake<User>>);

impl<'a> Widget for JoinButtons<'a> {
    fn create(self, msg: &mut GameMessage, event: &Event) -> Flow<bool> {
        let mut changed = false;

        msg.components.push(ActionRow::new(vec![
            event.button(
                Button::Action {
                    style: ButtonStyle::Success,
                    custom_id: "join".into(),
                    label: Some("Join".into()),
                    disabled: false,
                },
                |u| {
                    if !self.0.contains(&u) {
                        self.0.push(u);
                    }
                    changed = true
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
                    self.0.retain(|&o| o != u);
                    changed = true
                },
            ),
        ]));

        Flow::Return(changed)
    }
}

// OLD MENUS

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
                            rows.push(ActionRow::new(buttons));
                        }
                        return rows;
                    }
                }
            }
            rows.push(ActionRow::new(buttons));
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
                            rows.push(ActionRow::new(buttons));
                        }
                        return rows;
                    }
                }
            }
            rows.push(ActionRow::new(buttons));
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
