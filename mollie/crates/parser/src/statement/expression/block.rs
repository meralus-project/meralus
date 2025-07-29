use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Parse, ParseError, ParseResult, Parser, Statement};

pub fn parse_statements_until(parser: &mut Parser, token: &Token) -> ParseResult<(Vec<Positioned<Statement>>, Option<Positioned<Statement>>)> {
    let mut statements = Vec::new();
    let mut return_statement: Option<Positioned<Statement>> = None;

    while !parser.check(token) {
        let statement = Statement::parse(parser)?;

        if matches!(statement.value, Statement::Expression(_)) {
            if parser.try_consume(&Token::Semi) {
                statements.push(statement);
            } else if let Some(statement) = &return_statement {
                return Err(ParseError::new("missing ;", Some(statement.span)));
            } else {
                return_statement.replace(statement);
            }
        } else if return_statement.is_none() {
            statements.push(statement);
        } else {
            return Err(ParseError::new("return value already exists", Some(statement.span)));
        }
    }

    Ok((statements, return_statement))
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Block {
    pub statements: Vec<Positioned<Statement>>,
    pub final_statement: Option<Box<Positioned<Statement>>>,
}

impl Parse for Block {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let start = parser.consume(&Token::BraceOpen)?;

        let (statements, final_statement) = parse_statements_until(parser, &Token::BraceClose)?;

        let final_statement = final_statement.map(Box::new);
        let end = parser.consume(&Token::BraceClose)?;

        Ok(start.between(&end).wrap(Self { statements, final_statement }))
    }
}
