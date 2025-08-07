use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{CustomType, Ident, NameValue, Parse, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct EnumPathExpr {
    pub target: Positioned<Ident>,
    pub index: Positioned<CustomType>,
    pub properties: Option<Positioned<Vec<Positioned<NameValue>>>>,
}

impl Parse for EnumPathExpr {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.verify_if(Token::is_ident)?;
        parser.verify2(&Token::PathSep)?;

        let target = Ident::parse(parser)?;

        parser.consume(&Token::PathSep)?;

        let index = CustomType::parse(parser)?;

        let properties = parser.consume_separated_in(&Token::Comma, &Token::BraceOpen, &Token::BraceClose).ok();

        Ok(if let Some(properties) = &properties {
            target.between(properties)
        } else {
            target.between(&index)
        }
        .wrap(Self { target, index, properties }))
    }
}
