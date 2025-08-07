use mollie_parser::Block;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<Block> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        for statement in self.value.statements {
            if compiler.compile(chunk, statement)? {
                chunk.pop();
            }
        }

        let mut returns = false;

        if let Some(final_statement) = self.value.final_statement {
            compiler.compile(chunk, *final_statement)?;

            returns = true;
        }

        Ok(returns)
    }
}

impl GetType for Block {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        self.final_statement
            .as_ref()
            .map_or_else(|| Ok(void().into()), |final_statement| final_statement.get_type(compiler))
    }
}
