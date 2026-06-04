use crate::actions::CommandAction;

pub trait CommandParser {
    fn parse(&self, input: &str) -> CommandAction;
}

pub mod regex_parser;
pub mod ai_parser;