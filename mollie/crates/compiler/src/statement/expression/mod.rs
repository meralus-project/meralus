use mollie_parser::Expression;
use mollie_shared::{Positioned, Span};
use mollie_vm::Chunk;

use crate::{Compile, CompileResult, Compiler, GetType, TypeResult};

mod array;
mod block;
mod call;
mod ident;
mod index;
mod literal;
mod node;
mod type_index;

impl Compile for Positioned<Expression> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        use Expression::{Array, FunctionCall, Ident, Index, Literal, Node, This, TypeIndex};

        match self.value {
            Literal(value) => compiler.compile(chunk, self.span.wrap(value)),
            FunctionCall(value) => compiler.compile(chunk, self.span.wrap(value)),
            Node(value) => compiler.compile(chunk, self.span.wrap(value)),
            Array(value) => compiler.compile(chunk, self.span.wrap(value)),
            Index(value) => compiler.compile(chunk, self.span.wrap(value)),
            Ident(value) => compiler.compile(chunk, self.span.wrap(value)),
            TypeIndex(value) => compiler.compile(chunk, self.span.wrap(value)),
            This => {
                chunk.get_local(compiler.locals.get("self").unwrap().id);

                Ok(())
            }
        }
    }
}

impl GetType for Expression {
    fn get_type(&self, compiler: &Compiler, span: Span) -> TypeResult {
        match self {
            Self::Literal(value) => value.get_type(compiler, span),
            Self::FunctionCall(value) => value.get_type(compiler, span),
            Self::Node(value) => value.get_type(compiler, span),
            Self::Index(value) => value.get_type(compiler, span),
            Self::Array(value) => value.get_type(compiler, span),
            Self::TypeIndex(value) => value.get_type(compiler, span),
            Self::Ident(value) => value.get_type(compiler, span),
            Self::This => Ok(compiler.get_type("self")),
        }
    }
}
