use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Ident, NameWithGenerics, Parse, ParseResult, Parser, Type};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Property {
    pub name: Positioned<Ident>,
    pub nullable: Option<Positioned<bool>>,
    pub ty: Positioned<Type>,
}

impl Parse for Property {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct StructDecl {
    pub name: Positioned<NameWithGenerics>,
    pub properties: Positioned<Vec<Positioned<Property>>>,
}

impl Parse for StructDecl {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.consume(&Token::Struct)?;

        let name = NameWithGenerics::parse(parser)?;

        let properties = parser.consume_separated_in(&Token::Comma, &Token::BraceOpen, &Token::BraceClose)?;

        Ok(name.span.between(properties.span).wrap(Self { name, properties }))
    }
}
