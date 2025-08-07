use mollie_parser::Statement;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, void};

use crate::{Compile, CompileResult, Compiler, GetType, TypeResult};

mod component_decl;
mod enum_decl;
mod expression;
mod implementation;
mod struct_decl;
mod trait_decl;
mod variable_decl;

impl Compile for Positioned<Statement> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        use Statement::{ComponentDecl, EnumDecl, Expression, Impl, StructDecl, TraitDecl, VariableDecl};

        match self.value {
            Expression(value) => compiler.compile(chunk, self.span.wrap(value)),
            ComponentDecl(value) => compiler.compile(chunk, self.span.wrap(value)),
            StructDecl(value) => compiler.compile(chunk, self.span.wrap(value)),
            Impl(value) => compiler.compile(chunk, self.span.wrap(value)),
            TraitDecl(value) => compiler.compile(chunk, self.span.wrap(value)),
            VariableDecl(value) => compiler.compile(chunk, self.span.wrap(value)),
            EnumDecl(value) => compiler.compile(chunk, self.span.wrap(value)),
        }
    }
}

impl GetType for Statement {
    fn get_type(&self, compiler: &Compiler, span: Span) -> TypeResult {
        use Statement::{ComponentDecl, EnumDecl, Expression, Impl, StructDecl, TraitDecl, VariableDecl};

        match self {
            Expression(value) => value.get_type(compiler, span),
            Impl(value) => value.get_type(compiler, span),
            ComponentDecl(_) | StructDecl(_) | TraitDecl(_) | VariableDecl(_) | EnumDecl(_) => Ok(void().into()),
        }
    }
}
