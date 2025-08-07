use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Block, Expression, Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct IfElseExpression {
    pub condition: Box<Positioned<Expression>>,
    pub block: Positioned<Block>,
    pub else_block: Option<Box<Positioned<Expression>>>,
}

impl Parse for IfElseExpression {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let start = parser.consume(&Token::If)?;

        let condition = Expression::parse(parser)?;
        let block = Block::parse(parser)?;

        let else_block = if parser.try_consume(&Token::Else) {
            if parser.check(&Token::If) {
                Some(Self::parse(parser)?.map(Expression::IfElse))
            } else {
                Some(Block::parse(parser)?.map(Expression::Block))
            }
        } else {
            None
        }
        .map(Box::new);

        Ok(else_block
            .as_ref()
            .map_or_else(|| start.between(&block), |else_block| start.between(else_block))
            .wrap(Self {
                condition: Box::new(condition),
                block,
                else_block,
            }))
    }
}
