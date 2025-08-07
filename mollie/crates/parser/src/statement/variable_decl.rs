use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Expression, Ident, Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct VariableDecl {
    pub mutable: Option<Positioned<Token>>,
    pub name: Positioned<Ident>,
    pub value: Positioned<Expression>,
}

impl Parse for VariableDecl {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let start = parser.consume_one_of(&[Token::Const, Token::Let])?;
        let start_span = start.span;
        let mutable = if start.value == Token::Let { Some(start) } else { None };
        let name = Ident::parse(parser)?;

        parser.consume(&Token::Eq)?;

        let value = Expression::parse(parser)?;
        let end = parser.consume(&Token::Semi)?;

        Ok(start_span.between(end.span).wrap(Self { mutable, name, value }))
    }
}
