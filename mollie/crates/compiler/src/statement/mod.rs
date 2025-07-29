use mollie_parser::Statement;
use mollie_shared::{Positioned, Span};
use mollie_vm::Chunk;

use crate::{Compile, CompileResult, Compiler, GetType, TypeResult};

mod component_decl;
mod expression;
mod implementation;
mod struct_decl;
mod trait_decl;

impl Compile for Positioned<Statement> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        use Statement::{ComponentDecl, Expression, Impl, StructDecl, TraitDecl};

        match self.value {
            Expression(expression) => compiler.compile(chunk, self.span.wrap(expression)),
            ComponentDecl(component_decl) => compiler.compile(chunk, self.span.wrap(component_decl)),
            StructDecl(struct_decl) => compiler.compile(chunk, self.span.wrap(struct_decl)),
            Impl(implementation) => compiler.compile(chunk, self.span.wrap(implementation)),
            TraitDecl(trait_decl) => compiler.compile(chunk, self.span.wrap(trait_decl)),
        }
    }
}

impl GetType for Statement {
    fn get_type(&self, compiler: &Compiler, span: Span) -> TypeResult {
        match self {
            Self::Expression(value) => value.get_type(compiler, span),
            Self::ComponentDecl(value) => value.get_type(compiler, span),
            Self::StructDecl(value) => value.get_type(compiler, span),
            Self::Impl(value) => value.get_type(compiler, span),
            Self::TraitDecl(trait_decl) => trait_decl.get_type(compiler, span),
        }
    }
}
