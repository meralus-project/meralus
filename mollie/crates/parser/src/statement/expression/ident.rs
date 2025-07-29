use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Ident(pub String);

impl Ident {
    pub fn new<T: Into<String>>(value: T) -> Self {
        Self(value.into())
    }
}

impl Parse for Ident {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.consume_if(Token::is_ident).map(|value| value.map(Token::unwrap_ident).map(Self))
    }
}
