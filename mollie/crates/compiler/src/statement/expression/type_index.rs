use mollie_parser::TypeIndexExpr;
use mollie_shared::{Positioned, Span};
use mollie_vm::Chunk;

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<TypeIndexExpr> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        println!("ALOW");
        
        let ty = self.value.target.get_type(compiler)?;

        let vtable = compiler.vtables.get_index_of(&ty.variant).unwrap();
        let function = compiler.vtables[vtable].functions.get_index_of(&self.value.index.value.0).unwrap();

        chunk.get_type_function(vtable, function);

        Ok(())
    }
}

impl GetType for TypeIndexExpr {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        let ty = self.target.get_type(compiler)?;

        let vtable = compiler.vtables.get(&ty.variant).unwrap();

        Ok(vtable.functions.get(&self.index.value.0).unwrap().0.clone())
    }
}
