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
    Null,
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
            Token::Null => Some(Self::Null),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct NameWithGenerics {
    pub name: Positioned<Ident>,
    pub generics: Vec<Positioned<Ident>>,
}

impl Parse for NameWithGenerics {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let name = Ident::parse(parser)?;

        let (generics, end) = if parser.try_consume(&Token::Less) {
            let generics = parser.consume_separated_until(&Token::Comma, &Token::Greater)?;
            let end = parser.consume(&Token::Greater)?;

            (generics, end.span)
        } else {
            (Vec::new(), name.span)
        };

        Ok(name.span.between(end).wrap(Self { name, generics }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct CustomType {
    pub name: Positioned<Ident>,
    pub generics: Vec<Positioned<Type>>,
}

impl Parse for CustomType {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let name = Ident::parse(parser)?;

        let (generics, end) = if parser.try_consume(&Token::Less) {
            let generics = parser.consume_separated_until(&Token::Comma, &Token::Greater)?;
            let end = parser.consume(&Token::Greater)?;

            (generics, end.span)
        } else {
            (Vec::new(), name.span)
        };

        Ok(name.span.between(end).wrap(Self { name, generics }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum Type {
    Primitive(PrimitiveType),
    Custom(CustomType),
    Array(Box<Positioned<Self>>, Option<Positioned<usize>>),
    OneOf(Vec<Positioned<Self>>),
}

impl Parse for Type {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let value = PrimitiveType::parse(parser)
            .map(|v| v.map(Self::Primitive))
            .or_else(|_| CustomType::parse(parser).map(|v| v.map(Self::Custom)))?;

        let value = if parser.try_consume(&Token::BracketOpen) {
            let size = parser
                .consume_if(Token::is_integer)
                .map(|v| v.map(Token::unwrap_integer))
                .map_or(None, |size| size.value.0.try_into().ok().map(|v| size.span.wrap(v)));

            let end = parser.consume(&Token::BracketClose)?;

            value.span.between(end.span).wrap(Self::Array(Box::new(value), size))
        } else {
            value
        };

        if parser.try_consume(&Token::Or) {
            let mut types = vec![value, Self::parse(parser)?];

            while parser.try_consume(&Token::Or) {
                types.push(Self::parse(parser)?);
            }

            Ok(types[0].span.between(types[types.len() - 1].span).wrap(Self::OneOf(types)))
        } else {
            Ok(value)
        }
    }
}
