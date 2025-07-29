use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Ident, Node, Parse, ParseResult, Parser, Type};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct ComponentProperty {
    pub name: Positioned<Ident>,
    pub nullable: Option<Positioned<bool>>,
    pub ty: Positioned<Type>,
}

impl Parse for ComponentProperty {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.verify_if(Token::is_ident)?;
        parser.verify2_if(|token| matches!(token, Token::Colon | Token::Question))?;

        let name = Ident::parse(parser)?;
        let nullable = parser.consume(&Token::Question).map(|v| v.map(|_| true)).ok();

        parser.consume(&Token::Colon)?;

        let ty = Type::parse(parser)?;

        Ok(name.span.between(ty.span).wrap(Self { name, nullable, ty }))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct ComponentDecl {
    pub name: Positioned<Ident>,
    pub inherits: Option<Positioned<Ident>>,
    pub properties: Vec<Positioned<ComponentProperty>>,
    pub view: Option<Positioned<Node>>,
}

impl Parse for ComponentDecl {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.consume(&Token::Declare)?;

        let name = Ident::parse(parser)?;

        let inherits = if parser.try_consume(&Token::Inherits) {
            Some(Ident::parse(parser)?)
        } else {
            None
        };

        parser.consume(&Token::BraceOpen)?;

        let mut properties = Vec::new();

        loop {
            if !properties.is_empty() && parser.check2_if(Token::is_ident) && parser.check3(&Token::Colon) {
                parser.consume(&Token::Comma)?;
            }

            match ComponentProperty::parse(parser) {
                Ok(property) => properties.push(property),
                Err(_) => break,
            }
        }

        let view = if (parser.try_consume(&Token::Comma) || properties.is_empty()) && parser.check_if(Token::is_ident) {
            Some(Node::parse(parser)?)
        } else {
            None
        };

        let end = parser.consume(&Token::BraceClose)?;

        Ok(name.span.between(end.span).wrap(Self {
            name,
            inherits,
            properties,
            view,
        }))
    }
}
