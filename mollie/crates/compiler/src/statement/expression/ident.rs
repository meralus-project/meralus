use mollie_parser::Ident;
use mollie_shared::{Positioned, Span};
use mollie_vm::Chunk;

use crate::{Compile, CompileResult, Compiler, GetType, TypeError, TypeResult};

impl Compile for Positioned<Ident> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let (frame, id) = compiler.get_var_index(&self.value.0).unwrap();

        if let Some(value) = compiler.assign.take() {
            compiler.compile(chunk, value)?;

            chunk.set_local(id);

            Ok(false)
        } else {
            chunk.get_local(frame, id);

            Ok(true)
        }
    }
}

impl GetType for Ident {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        compiler
            .types
            .get(&self.0)
            .or_else(|| compiler.get_var(&self.0).map(|v| &v.ty))
            .cloned()
            .ok_or_else(|| TypeError::NotFound {
                ty: None,
                name: self.0.clone(),
            })
    }
}
