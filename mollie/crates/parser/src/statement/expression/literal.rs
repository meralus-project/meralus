use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Number {
    I64(i64),
    F32(f32),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum SizeType {
    Pixel,
    Percent,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Literal {
    SizeUnit(Number, SizeType),
    Number(Number),
    Boolean(bool),
    String(String),
    Null,
}

impl Parse for Literal {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser
            .consume_if(|token| {
                matches!(
                    token,
                    Token::Boolean(_) | Token::Integer(..) | Token::String(_) | Token::Float(..) | Token::Null
                )
            })
            .map(|token| {
                token.span.wrap(match token.value {
                    Token::Boolean(value) => Self::Boolean(value),
                    Token::Integer(value, ..) => Self::Number(Number::I64(value)),
                    Token::Float(value, _) => Self::Number(Number::F32(value)),
                    Token::String(value) => Self::String(value),
                    Token::Null => Self::Null,
                    _ => unreachable!(),
                })
            })
    }
}
