use mollie_parser::IfElseExpression;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, Inst, boolean};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile for Positioned<IfElseExpression> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let condition_type = self.value.condition.get_type(compiler)?;

        if !condition_type.variant.same_as(&boolean(), &compiler.generics) {
            return Err(TypeError::Unexpected {
                expected: Box::new(boolean().kind().into()),
                got: Box::new(condition_type.kind()),
            }
            .into());
        }

        chunk.push_frame();
        compiler.push_frame();

        self.value.condition.compile(chunk, compiler)?;

        let start = chunk.len();

        chunk.jump_if_false(0);

        let returns = self.value.block.compile(chunk, compiler)?;

        chunk[start] = Inst::JumpIfFalse(chunk.len() - start);

        if let Some(else_block) = self.value.else_block {
            chunk[start] = Inst::JumpIfFalse(chunk.len() - start + 1);

            let start = chunk.len();

            chunk.jump(0);

            else_block.compile(chunk, compiler)?;

            chunk[start] = Inst::Jump(chunk.len().cast_signed() - start.cast_signed());
        }

        compiler.pop_frame();
        chunk.pop_frame();

        Ok(returns)
    }
}

impl GetType for IfElseExpression {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        self.block.get_type(compiler)
    }
}
