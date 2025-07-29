mod component_decl;
mod expression;
mod implementation;
mod struct_decl;
mod trait_decl;

use mollie_shared::Positioned;

pub use self::{
    component_decl::{ComponentDecl, ComponentProperty},
    expression::*,
    implementation::{Argument, Impl, ImplFunction},
    struct_decl::{Property, StructDecl},
    trait_decl::{TraitDecl, TraitFuncArgument, TraitFunction},
};
use super::{ParseResult, Parser};
use crate::Parse;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Statement {
    Expression(Expression),
    StructDecl(StructDecl),
    ComponentDecl(ComponentDecl),
    TraitDecl(TraitDecl),
    Impl(Impl),
}

impl Parse for Statement {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        ComponentDecl::parse(parser)
            .map(|v| v.map(Self::ComponentDecl))
            .or_else(|_| StructDecl::parse(parser).map(|v| v.map(Self::StructDecl)))
            .or_else(|_| Impl::parse(parser).map(|v| v.map(Self::Impl)))
            .or_else(|_| TraitDecl::parse(parser).map(|v| v.map(Self::TraitDecl)))
            .or_else(|_| Expression::parse(parser).map(|v| v.map(Self::Expression)))
    }
}
