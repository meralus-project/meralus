use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{CustomType, Expression, Ident, Literal, Parse, ParseError, ParseResult, Parser};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct NameValuePattern {
    pub name: Positioned<Ident>,
    pub value: Option<Positioned<AsPattern>>,
}

impl Parse for NameValuePattern {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let name = Ident::parse(parser)?;
        let value = if parser.try_consume(&Token::Colon) {
            Some(AsPattern::parse(parser)?)
        } else {
            None
        };

        Ok(if let Some(value) = &value { name.between(value) } else { name.span }.wrap(Self { name, value }))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum AsPattern {
    Literal(Literal),
    Enum {
        target: Positioned<Ident>,
        index: Positioned<CustomType>,
        values: Option<Positioned<Vec<Positioned<NameValuePattern>>>>,
    },
    TypeName {
        ty: Positioned<CustomType>,
        name: Positioned<Ident>,
    },
}

impl Parse for AsPattern {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        Literal::parse(parser)
            .map(|v| v.map(Self::Literal))
            .or_else(|_| {
                parser.verify_if(Token::is_ident)?;
                parser.verify2(&Token::PathSep)?;

                let target = Ident::parse(parser)?;

                parser.consume(&Token::PathSep)?;

                let index = CustomType::parse(parser)?;

                let values = parser.consume_separated_in(&Token::Comma, &Token::BraceOpen, &Token::BraceClose).ok();

                Ok(if let Some(values) = &values {
                    target.between(values)
                } else {
                    target.between(&index)
                }
                .wrap(Self::Enum { target, index, values }))
            })
            .or_else(|_: ParseError| {
                let ty = CustomType::parse(parser)?;
                let name = Ident::parse(parser)?;

                Ok(ty.between(&name).wrap(Self::TypeName { ty, name }))
            })
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct AsExpr {
    pub target: Box<Positioned<Expression>>,
    pub pattern: Positioned<AsPattern>,
}

// impl Parse for AsExpr {
//     fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
//         parser.verify_if(Token::is_ident)?;
//         parser.verify2(&Token::As)?;

//         let target = Ident::parse(parser)?;

//         parser.consume(&Token::As)?;

//         let pattern = AsPattern::parse(parser)?;

//         Ok(target.between(&pattern).wrap(Self { target, pattern }))
//     }
// }
