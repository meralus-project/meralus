use mollie_parser::VariableDecl;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<VariableDecl> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let ty = self.value.value.get_type(compiler)?;

        println!("variable {ty:#?}");

        let id = compiler.var(&self.value.name.value.0, ty);

        compiler.compile(chunk, self.value.value)?;
        chunk.set_local(id);

        Ok(false)
    }
}

impl GetType for VariableDecl {
    fn get_type(&self, _: &Compiler, _: Span) -> TypeResult {
        Ok(void().into())
    }
}
