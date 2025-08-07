use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Ident, NameWithGenerics, Parse, ParseResult, Parser, Property};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct EnumVariant {
    pub name: Positioned<Ident>,
    pub properties: Option<Positioned<Vec<Positioned<Property>>>>,
}

impl Parse for EnumVariant {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.verify_if(Token::is_ident)?;

        let name = Ident::parse(parser)?;
        let properties = parser.consume_separated_in(&Token::Comma, &Token::BraceOpen, &Token::BraceClose).ok();

        Ok(if let Some(properties) = &properties {
            name.span.between(properties.span)
        } else {
            name.span
        }
        .wrap(Self { name, properties }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct EnumDecl {
    pub name: Positioned<NameWithGenerics>,
    pub variants: Positioned<Vec<Positioned<EnumVariant>>>,
}

impl Parse for EnumDecl {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.consume(&Token::Enum)?;

        let name = NameWithGenerics::parse(parser)?;

        let variants = parser.consume_separated_in(&Token::Comma, &Token::BraceOpen, &Token::BraceClose)?;

        Ok(name.span.between(variants.span).wrap(Self { name, variants }))
    }
}
