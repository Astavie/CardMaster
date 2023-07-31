use discord::{
    interaction::{Interaction, MessageComponent},
    message::{ActionRow, ActionRowComponent, Button, ButtonStyle, SelectOption, TextSelectMenu},
    resource::Snowflake,
    user::User,
};
use monostate::MustBeU64;

use crate::game::Flow;

pub struct Setup {
    pub options: Vec<(String, SetupOption)>,
}

const B64_TABLE: [char; 64] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l',
    'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '+', '/',
];

impl Setup {
    pub fn render(&self) -> Vec<ActionRow> {
        assert!(self.options.len() <= 5);
        self.options
            .iter()
            .enumerate()
            .map(|(oi, (name, option))| ActionRow {
                typ: MustBeU64::<1>,
                components: match *option {
                    SetupOption::MultiSelect(ref menu) => {
                        assert!(menu.len() <= 64);
                        vec![ActionRowComponent::TextSelectMenu(TextSelectMenu {
                            custom_id: format!("{}", B64_TABLE[oi]),
                            options: menu
                                .iter()
                                .enumerate()
                                .map(|(i, &(ref name, enabled))| SelectOption {
                                    label: name.clone(),
                                    value: format!("{}", B64_TABLE[i]),
                                    description: None,
                                    default: enabled,
                                })
                                .collect(),
                            placeholder: Some(name.clone()),
                            min_values: 0,
                            max_values: menu.len(),
                            disabled: false,
                        })]
                    }
                    SetupOption::Flags(ref menu) => {
                        assert!(menu.len() <= 4);
                        let mut buttons = vec![ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: format!("_label_{}", B64_TABLE[oi]),
                            label: Some(name.clone()),
                            disabled: true,
                        })];
                        buttons.extend(menu.iter().enumerate().map(|(i, &(ref name, enabled))| {
                            ActionRowComponent::Button(Button::Action {
                                style: if enabled {
                                    // Green
                                    ButtonStyle::Success
                                } else {
                                    // Gray
                                    ButtonStyle::Secondary
                                },
                                custom_id: format!("{}{}", B64_TABLE[oi], B64_TABLE[i]),
                                label: Some(name.clone()),
                                disabled: false,
                            })
                        }));
                        buttons
                    }
                    SetupOption::Number(min, max, val) => vec![
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: format!("_label_{}", B64_TABLE[oi]),
                            label: Some(name.clone()),
                            disabled: true,
                        }),
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: format!("{}d", B64_TABLE[oi]),
                            label: Some("<".to_owned()),
                            disabled: val <= min,
                        }),
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Secondary,
                            custom_id: format!("{}v", B64_TABLE[oi]),
                            label: Some(val.to_string()),
                            disabled: false,
                        }),
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Primary,
                            custom_id: format!("{}i", B64_TABLE[oi]),
                            label: Some(">".to_owned()),
                            disabled: val >= max,
                        }),
                    ],
                    SetupOption::Players(_) => vec![
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Success,
                            custom_id: format!("{}j", B64_TABLE[oi]),
                            label: Some("Join".into()),
                            disabled: false,
                        }),
                        ActionRowComponent::Button(Button::Action {
                            style: ButtonStyle::Danger,
                            custom_id: format!("{}l", B64_TABLE[oi]),
                            label: Some("Leave".into()),
                            disabled: false,
                        }),
                    ],
                },
            })
            .collect()
    }

    pub fn update(&mut self, it: &Interaction<MessageComponent>) -> Flow<()> {
        // update state
        let mut chars = it.data.custom_id.chars();
        let ob = chars.next()?;
        let oi = B64_TABLE.iter().position(|&c| c == ob)?;
        let option = &mut self.options.get_mut(oi)?.1;

        match *option {
            SetupOption::MultiSelect(ref mut menu) => {
                for (_, option) in menu.iter_mut() {
                    *option = false;
                }
                for select in it.data.values.iter() {
                    let Some(b) = select.chars().next() else { continue };
                    let Some(i) = B64_TABLE.iter().position(|&c| c == b) else { continue };
                    let Some(option) = menu.get_mut(i).map(|(_, b)| b) else { continue };
                    *option = true;
                }
            }
            SetupOption::Flags(ref mut menu) => {
                let b = chars.next()?;
                let i = B64_TABLE.iter().position(|&c| c == b)?;
                let option = &mut menu.get_mut(i)?.1;
                *option = !*option;
            }
            SetupOption::Number(min, max, ref mut val) => match chars.next()? {
                'd' if *val > min => {
                    *val = *val - 1;
                }
                'i' if *val < max => {
                    *val = *val + 1;
                }
                _ => None?,
            },
            SetupOption::Players(ref mut vec) => match chars.next()? {
                'j' => match vec.iter().position(|&u| u == it.user.id) {
                    None => vec.push(it.user.id),
                    _ => None?,
                },
                'l' => {
                    let pos = vec.iter().position(|&u| u == it.user.id)?;
                    vec.remove(pos);
                }
                _ => None?,
            },
        }

        Flow::Return(())
    }
}

pub enum SetupOption {
    MultiSelect(Vec<(String, bool)>),
    Flags(Vec<(String, bool)>),
    Number(i32, i32, i32),
    Players(Vec<Snowflake<User>>),
}
