use mollie_parser::Ident;
use mollie_shared::{Positioned, Span};
use mollie_vm::Chunk;

use crate::{Compile, CompileResult, Compiler, GetType, TypeResult};

impl Compile for Positioned<Ident> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        chunk.get_local(compiler.locals.get(&self.value.0).unwrap().id);

        Ok(())
    }
}

impl GetType for Ident {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        println!("{}", self.0);
        
        Ok(compiler.locals.get(&self.0).unwrap().ty.clone().into())
    }
}
