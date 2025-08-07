use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Block, CustomType, Ident, Parse, ParseResult, Parser, ty::Type};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Argument {
    pub name: Positioned<Ident>,
    pub ty: Positioned<Type>,
}

impl Parse for Argument {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        parser.verify_if(Token::is_ident)?;
        parser.verify2(&Token::Colon)?;

        let name = Ident::parse(parser)?;

        parser.consume(&Token::Colon)?;

        let ty = Type::parse(parser)?;

        Ok(name.between(&ty).wrap(Self { name, ty }))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct ImplFunction {
    pub name: Positioned<Ident>,
    pub this: Option<Positioned<Token>>,
    pub args: Vec<Positioned<Argument>>,
    pub returns: Option<Positioned<Type>>,
    pub body: Positioned<Block>,
}

impl Parse for ImplFunction {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let start = parser.consume(&Token::Fn)?;

        let name = Ident::parse(parser)?;

        parser.consume(&Token::ParenOpen)?;

        let this = parser.consume(&Token::This).ok();

        if this.is_some() {
            parser.try_consume(&Token::Comma);
        }

        let mut args = Vec::new();

        while !parser.check(&Token::ParenClose) {
            if !args.is_empty() {
                parser.consume(&Token::Comma)?;
            }

            if parser.check(&Token::ParenClose) {
                break;
            }

            args.push(Argument::parse(parser)?);
        }

        parser.consume(&Token::ParenClose)?;

        let returns = if parser.try_consume(&Token::Arrow) {
            Some(Type::parse(parser)?)
        } else {
            None
        };

        let body = Block::parse(parser)?;

        Ok(start.between(&body).wrap(Self {
            name,
            this,
            args,
            returns,
            body,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Impl {
    pub generics: Vec<Positioned<Ident>>,
    pub trait_name: Option<Positioned<CustomType>>,
    pub target: Positioned<Type>,
    pub functions: Positioned<Vec<Positioned<ImplFunction>>>,
}

impl Parse for Impl {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let start = parser.consume(&Token::Impl)?;

        let generics = if parser.try_consume(&Token::Less) {
            let generics = parser.consume_separated_until(&Token::Comma, &Token::Greater)?;

            parser.consume(&Token::Greater)?;

            generics
        } else {
            Vec::new()
        };

        let trait_name = if parser.try_consume(&Token::Trait) {
            let name = CustomType::parse(parser)?;

            parser.consume(&Token::For)?;

            Some(name)
        } else {
            None
        };

        let target = Type::parse(parser)?;

        let functions = parser.consume_in(&Token::BraceOpen, &Token::BraceClose)?;

        Ok(start.between(&functions).wrap(Self {
            generics,
            trait_name,
            target,
            functions,
        }))
    }
}
