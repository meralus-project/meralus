use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Ident, Parse, ParseResult, Parser, Type};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct TypeIndexExpr {
    pub target: Positioned<Type>,
    pub index: Positioned<Ident>,
}

impl Parse for TypeIndexExpr {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let start = parser.consume(&Token::Less)?;
        let target = Type::parse(parser)?;

        parser.consume(&Token::Greater)?;
        parser.consume(&Token::PathSep)?;

        let index = Ident::parse(parser)?;

        Ok(start.between(&index).wrap(Self { target, index }))
    }
}
