use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{CustomType, Expression, Ident, Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct NameValue {
    pub name: Positioned<Ident>,
    pub value: Positioned<Expression>,
}

impl Parse for NameValue {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.verify_if(Token::is_ident)?;
        parser.verify2(&Token::Colon)?;

        let name = Ident::parse(parser)?;

        parser.consume(&Token::Colon)?;

        let value = Expression::parse(parser)?;

        Ok(name.span.between(value.span).wrap(Self { name, value }))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Node {
    pub name: Positioned<CustomType>,
    pub from: Option<(Positioned<Box<Expression>>, Positioned<Ident>)>,
    pub properties: Vec<Positioned<NameValue>>,
    pub children: Positioned<Vec<Positioned<Self>>>,
}

impl Parse for Node {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.verify_if(Token::is_ident)?;
        parser.verify2_if(|t| matches!(t, Token::BraceOpen | Token::From | Token::Less))?;

        let name = CustomType::parse(parser)?;

        let from = if parser.try_consume(&Token::From) {
            parser.consume(&Token::Less)?;

            let target = Expression::parse(parser)?.map(Box::new);

            parser.consume(&Token::As)?;

            let ty = Ident::parse(parser)?;

            parser.consume(&Token::Greater)?;

            Some((target, ty))
        } else {
            None
        };

        parser.consume(&Token::BraceOpen)?;

        let mut properties = Vec::new();

        loop {
            if !properties.is_empty() && parser.check2_if(Token::is_ident) && parser.check3(&Token::Colon) {
                parser.consume(&Token::Comma)?;
            }

            match NameValue::parse(parser) {
                Ok(property) => properties.push(property),
                Err(_) => break,
            }
        }

        let mut children = Vec::new();

        if (parser.try_consume(&Token::Comma) || properties.is_empty()) && parser.check_if(Token::is_ident) {
            while !parser.check(&Token::BraceClose) {
                children.push(Self::parse(parser)?);
            }
        }

        let end = parser.consume(&Token::BraceClose)?;

        let children = if children.is_empty() {
            name.span.between(end.span).wrap(children)
        } else {
            children[0].span.between(children[children.len() - 1].span).wrap(children)
        };

        Ok(name.span.between(end.span).wrap(Self {
            name,
            from,
            properties,
            children,
        }))
    }
}
