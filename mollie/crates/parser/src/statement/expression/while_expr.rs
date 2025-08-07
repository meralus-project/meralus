use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Block, Expression, Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct WhileExpression {
    pub condition: Box<Positioned<Expression>>,
    pub block: Positioned<Block>,
}

impl Parse for WhileExpression {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let start = parser.consume(&Token::While)?;

        let condition = Expression::parse(parser)?;
        let block = Block::parse(parser)?;

        Ok(start.between(&block).wrap(Self {
            condition: Box::new(condition),
            block,
        }))
    }
}
