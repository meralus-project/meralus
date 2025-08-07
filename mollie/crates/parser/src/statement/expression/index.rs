use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Expression, Ident, Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum IndexTarget {
    Named(Ident),
    Expression(Box<Expression>),
}

impl Parse for IndexTarget {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        if parser.try_consume(&Token::Dot) {
            Ident::parse(parser).map(|v| v.map(Self::Named))
        } else {
            let start = parser.consume(&Token::BracketOpen)?;
            let value = Expression::parse(parser).map(|v| Self::Expression(Box::new(v.value)))?;
            let end = parser.consume(&Token::BracketClose)?;

            Ok(start.between(&end).wrap(value))
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct IndexExpr {
    pub target: Box<Positioned<Expression>>,
    pub index: Positioned<IndexTarget>,
}

impl IndexExpr {
    /// # Errors
    ///
    /// Returns error if parsing failed
    pub fn parse(parser: &mut Parser, target: Positioned<Expression>) -> ParseResult<Positioned<Self>> {
        let index = IndexTarget::parse(parser)?;

        Ok(target.between(&index).wrap(Self {
            target: Box::new(target),
            index,
        }))
    }
}
