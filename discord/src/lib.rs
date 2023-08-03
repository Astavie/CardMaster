#![feature(return_position_impl_trait_in_trait)]
#![feature(associated_type_defaults)]

pub mod gateway;
pub mod request;
pub mod resource;

pub mod application;
pub mod channel;
pub mod command;
pub mod guild;
pub mod interaction;
pub mod message;
pub mod user;

pub struct EscapedChars<T: Iterator<Item = char>>(T, Option<char>);

impl<T: Iterator<Item = char>> EscapedChars<T> {
    pub fn new(t: T) -> Self {
        Self(t, None)
    }
}

impl<T: Iterator<Item = char>> Iterator for EscapedChars<T> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        match self.1.take() {
            Some(c) => Some(c),
            None => match self.0.next() {
                c
                @ Some('\\' | '_' | '*' | '|' | '~' | '`' | '[' | ']' | '(' | ')' | '<' | '>') => {
                    self.1 = c;
                    Some('\\')
                }
                c => c,
            },
        }
    }
}

pub fn escape_string(s: &str) -> String {
    EscapedChars::new(s.chars()).collect()
}
