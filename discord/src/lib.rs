use std::fmt::{self, Write};

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

#[derive(PartialEq, Eq)]
pub enum InlineCodeState {
    None,
    Starting,
    Inside,
    Ended,
}

pub struct DiscordFormatter<'a> {
    fmt: &'a mut (dyn Write + 'a),
    state: InlineCodeState,
}

impl<'a> DiscordFormatter<'a> {
    pub fn new(fmt: &'a mut (dyn Write + 'a)) -> Self {
        Self {
            fmt,
            state: InlineCodeState::None,
        }
    }
    pub fn start_code(&mut self) -> fmt::Result {
        match self.state {
            InlineCodeState::None => {
                self.state = InlineCodeState::Starting;
                Ok(())
            }
            InlineCodeState::Starting => Ok(()),
            InlineCodeState::Inside => Ok(()),
            InlineCodeState::Ended => {
                self.fmt.write_str(" ")?;
                self.state = InlineCodeState::Starting;
                Ok(())
            }
        }
    }
    pub fn end_code(&mut self) -> fmt::Result {
        match self.state {
            InlineCodeState::None => Ok(()),
            InlineCodeState::Starting => {
                self.state = InlineCodeState::None;
                Ok(())
            }
            InlineCodeState::Inside => {
                self.fmt.write_str("``")?;
                self.state = InlineCodeState::Ended;
                Ok(())
            }
            InlineCodeState::Ended => Ok(()),
        }
    }
    pub fn unescaped(&mut self) -> &mut (dyn Write + 'a) {
        self.fmt
    }
}

impl Write for DiscordFormatter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        match self.state {
            InlineCodeState::None => {
                for c in EscapedChars::new(s.chars()) {
                    self.fmt.write_char(c)?
                }
                Ok(())
            }
            InlineCodeState::Starting => {
                if !s.is_empty() {
                    self.fmt.write_str("``")?;
                    self.fmt.write_str(s)?;
                    self.state = InlineCodeState::Inside
                }
                Ok(())
            }
            InlineCodeState::Inside => self.fmt.write_str(s),
            InlineCodeState::Ended => {
                if !s.is_empty() {
                    for c in EscapedChars::new(s.chars()) {
                        self.fmt.write_char(c)?
                    }
                    self.state = InlineCodeState::None
                }
                Ok(())
            }
        }
    }
}

pub trait DisplayDiscord {
    fn fmt(&self, f: &mut DiscordFormatter<'_>) -> fmt::Result;
}
