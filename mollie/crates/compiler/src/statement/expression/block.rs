use mollie_parser::Block;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile<Chunk> for Positioned<Block> {
    fn compile(self, _: &mut Chunk, compiler: &mut Compiler) -> CompileResult<Chunk> {
        let mut chunk = Chunk::default();

        for statement in self.value.statements {
            compiler.compile(&mut chunk, statement)?;
        }

        if let Some(final_statement) = self.value.final_statement {
            compiler.compile(&mut chunk, *final_statement)?;

            chunk.ret();
        }

        chunk.halt();

        Ok(chunk)
    }
}

impl GetType for Positioned<Block> {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        self.value
            .final_statement
            .as_ref()
            .map_or_else(|| Ok(void().into()), |final_statement| final_statement.get_type(compiler))
    }
}
