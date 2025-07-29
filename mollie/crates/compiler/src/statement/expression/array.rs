use mollie_parser::ArrayExpr;
use mollie_shared::{Positioned, Span};
use mollie_vm::{ArrayType, Chunk, ComplexType, Type, TypeVariant};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<ArrayExpr> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let size = self.value.elements.len();

        for element in self.value.elements {
            compiler.compile(chunk, element)?;
        }

        chunk.create_array(size);

        Ok(())
    }
}

impl GetType for ArrayExpr {
    fn get_type(&self, compiler: &Compiler, span: Span) -> TypeResult {
        let element = self.elements[0].get_type(compiler)?;
        let size = self.elements.len();

        Ok(Type {
            variant: TypeVariant::complex(ComplexType::Array(ArrayType { element, size: Some(size) })),
            declared_at: Some(span),
        })
    }
}
