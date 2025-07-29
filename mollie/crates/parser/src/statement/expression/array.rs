use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Expression, Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct ArrayExpr {
    pub elements: Vec<Positioned<Expression>>,
}

impl Parse for ArrayExpr {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser
            .consume_separated_in(&Token::Comma, &Token::BracketOpen, &Token::BracketClose)
            .map(|elements| elements.map(|elements| Self { elements }))
    }
}
