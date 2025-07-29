use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Ident, Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum PrimitiveType {
    Integer,
    Float,
    Boolean,
    String,
    Component,
    Void,
}

impl Parse for PrimitiveType {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.consume_map(|token| match token {
            Token::Ident(value) => match value.as_str() {
                "integer" => Some(Self::Integer),
                "float" => Some(Self::Float),
                "boolean" => Some(Self::Boolean),
                "string" => Some(Self::String),
                "component" => Some(Self::Component),
                "void" => Some(Self::Void),
                _ => None,
            },
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum Type {
    Primitive(PrimitiveType),
    Custom(Ident),
    Array(Box<Positioned<Self>>, Option<Positioned<usize>>),
}

impl Parse for Type {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let value = PrimitiveType::parse(parser)
            .map(|v| v.map(Self::Primitive))
            .or_else(|_| Ident::parse(parser).map(|v| v.map(Self::Custom)))?;

        if parser.try_consume(&Token::BracketOpen) {
            let size = parser
                .consume_if(Token::is_integer)
                .map(|v| v.map(Token::unwrap_integer))
                .map_or(None, |size| Some(size.map(|v| v.0 as usize)));

            let end = parser.consume(&Token::BracketClose)?;

            Ok(value.span.between(end.span).wrap(Self::Array(Box::new(value), size)))
        } else {
            Ok(value)
        }
    }
}
