use mollie_parser::TypeIndexExpr;
use mollie_shared::{Positioned, Span};
use mollie_vm::Chunk;

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<TypeIndexExpr> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let ty = self.value.target.get_type(compiler)?;

        let (vtable, trait_index, function) = compiler.find_vtable_function_index(&ty.variant, &self.value.index.value.0).unwrap();

        chunk.get_type_function(vtable, trait_index, function);

        Ok(false)
    }
}

impl GetType for TypeIndexExpr {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        let ty = self.target.get_type(compiler)?;

        let (vtable, trait_index, function) = compiler.find_vtable_function_index(&ty.variant, &self.index.value.0).unwrap();

        Ok(compiler.vtables[vtable][&trait_index][function].0.clone())
    }
}
