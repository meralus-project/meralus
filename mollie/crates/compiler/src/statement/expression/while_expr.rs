use mollie_parser::WhileExpression;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, Inst, boolean};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile for Positioned<WhileExpression> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let condition_type = self.value.condition.get_type(compiler)?;

        if !condition_type.variant.same_as(&boolean(), &compiler.generics) {
            return Err(TypeError::Unexpected {
                expected: Box::new(boolean().kind().into()),
                got: Box::new(condition_type.kind()),
            }
            .into());
        }

        let loop_start = chunk.len();

        chunk.push_frame();
        compiler.push_frame();

        self.value.condition.compile(chunk, compiler)?;

        let start = chunk.len();

        chunk.jump_if_false(0);

        let returns = self.value.block.compile(chunk, compiler)?;

        compiler.pop_frame();
        chunk.pop_frame();

        chunk.jump(-((chunk.len() - loop_start) as isize));

        chunk[start] = Inst::JumpIfFalse(chunk.len() - start);

        chunk.pop_frame();

        Ok(returns)
    }
}

impl GetType for WhileExpression {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        self.block.get_type(compiler)
    }
}
