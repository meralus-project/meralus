use mollie_lexer::Token;
use mollie_shared::Positioned;

use crate::{Ident, NameWithGenerics, Parse, ParseResult, Parser, ty::Type};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct TraitFuncArgument {
    pub name: Positioned<Ident>,
    pub ty: Positioned<Type>,
}

impl Parse for TraitFuncArgument {
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
pub struct TraitFunction {
    pub name: Positioned<Ident>,
    pub this: Option<Positioned<Token>>,
    pub args: Vec<Positioned<TraitFuncArgument>>,
    pub returns: Option<Positioned<Type>>,
}

impl Parse for TraitFunction {
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

            args.push(TraitFuncArgument::parse(parser)?);
        }

        parser.consume(&Token::ParenClose)?;

        let returns = if parser.try_consume(&Token::Arrow) {
            Some(Type::parse(parser)?)
        } else {
            None
        };

        let end = parser.consume(&Token::Semi)?;

        Ok(start.between(&end).wrap(Self { name, this, args, returns }))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct TraitDecl {
    pub name: Positioned<NameWithGenerics>,
    pub functions: Positioned<Vec<Positioned<TraitFunction>>>,
}

impl Parse for TraitDecl {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        let start = parser.consume(&Token::Trait)?;

        let name = NameWithGenerics::parse(parser)?;

        let functions = parser.consume_in(&Token::BraceOpen, &Token::BraceClose)?;

        Ok(start.between(&functions).wrap(Self { name, functions }))
    }
}
