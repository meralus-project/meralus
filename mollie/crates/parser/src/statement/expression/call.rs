use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Expression, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct FunctionCall {
    pub function: Box<Positioned<Expression>>,
    pub args: Positioned<Vec<Positioned<Expression>>>,
}

impl FunctionCall {
    /// # Errors
    ///
    /// Returns error if parsing failed
    pub fn parse(parser: &mut Parser, target: Positioned<Expression>) -> ParseResult<Positioned<Self>> {
        let args = parser.consume_separated_in::<Expression>(&Token::Comma, &Token::ParenOpen, &Token::ParenClose)?;

        Ok(target.between(&args).wrap(Self {
            function: Box::new(target),
            args,
        }))
    }
}
