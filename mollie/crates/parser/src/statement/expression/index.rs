use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Expression, Ident, Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct IndexExpr {
    pub target: Box<Positioned<Expression>>,
    pub index: Positioned<Ident>,
}

impl IndexExpr {
    pub fn parse(parser: &mut Parser, target: Positioned<Expression>) -> ParseResult<Positioned<Self>> {
        parser.consume(&Token::Dot)?;

        let index = Ident::parse(parser)?;

        Ok(target.between(&index).wrap(Self {
            target: Box::new(target),
            index,
        }))
    }
}
