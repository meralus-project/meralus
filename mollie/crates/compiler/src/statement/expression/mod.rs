use mollie_parser::Expression;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, boolean};

use crate::{Compile, CompileResult, Compiler, GetType, TypeResult};

mod array;
mod as_expr;
mod binary;
mod block;
mod call;
mod enum_path;
mod ident;
mod if_else;
mod index;
mod literal;
mod node;
mod type_index;
mod while_expr;

impl Compile for Positioned<Expression> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        use Expression::{Array, AsExpr, Binary, Block, EnumPath, FunctionCall, Ident, IfElse, Index, Literal, Node, This, TypeIndex, While};

        match self.value {
            Literal(value) => compiler.compile(chunk, self.span.wrap(value)),
            FunctionCall(value) => compiler.compile(chunk, self.span.wrap(value)),
            Node(value) => compiler.compile(chunk, self.span.wrap(value)),
            Array(value) => compiler.compile(chunk, self.span.wrap(value)),
            Index(value) => compiler.compile(chunk, self.span.wrap(value)),
            IfElse(value) => compiler.compile(chunk, self.span.wrap(value)),
            Binary(value) => compiler.compile(chunk, self.span.wrap(value)),
            While(value) => compiler.compile(chunk, self.span.wrap(value)),
            EnumPath(value) => compiler.compile(chunk, self.span.wrap(value)),
            AsExpr(value) => compiler.compile(chunk, self.span.wrap(value)),
            Block(value) => {
                chunk.push_frame();
                compiler.push_frame();

                let returns = compiler.compile(chunk, self.span.wrap(value))?;

                compiler.pop_frame();
                chunk.pop_frame();

                Ok(returns)
            }
            Ident(value) => compiler.compile(chunk, self.span.wrap(value)),
            TypeIndex(value) => compiler.compile(chunk, self.span.wrap(value)),
            This => {
                let (frame, id) = compiler.get_var_index("self").unwrap();

                chunk.get_local(frame, id);

                Ok(true)
            }
        }
    }
}

impl GetType for Expression {
    fn get_type(&self, compiler: &Compiler, span: Span) -> TypeResult {
        use Expression::{Array, AsExpr, Binary, Block, EnumPath, FunctionCall, Ident, IfElse, Index, Literal, Node, This, TypeIndex, While};

        match self {
            Literal(value) => value.get_type(compiler, span),
            FunctionCall(value) => value.get_type(compiler, span),
            Node(value) => value.get_type(compiler, span),
            IfElse(value) => value.get_type(compiler, span),
            Index(value) => value.get_type(compiler, span),
            Binary(value) => value.get_type(compiler, span),
            EnumPath(value) => value.get_type(compiler, span),
            Block(value) => value.get_type(compiler, span),
            Array(value) => value.get_type(compiler, span),
            TypeIndex(value) => value.get_type(compiler, span),
            Ident(value) => value.get_type(compiler, span),
            While(value) => value.get_type(compiler, span),
            AsExpr(_) => Ok(boolean().into()),
            This => compiler.get_local_type("self"),
        }
    }
}
