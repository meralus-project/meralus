use mollie_parser::StructDecl;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ComplexType, StructType, Type, TypeVariant, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<StructDecl> {
    fn compile(self, _: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let mut properties = Vec::new();

        for property in &self.value.properties.value {
            // let nullable = property.value.nullable.is_some();
            let name = &property.value.name.value.0;
            let ty = property.value.ty.get_type(compiler)?;

            properties.push((name.clone(), /* nullable, */ ty));
        }

        let ty = Type {
            variant: TypeVariant::complex(ComplexType::Struct(StructType { properties })),
            declared_at: Some(self.span),
        };

        compiler.types.insert(self.value.name.value.0, ty);

        // compiler.var(self.value.name.value.0, ty.variant);

        Ok(())
    }
}

impl GetType for StructDecl {
    fn get_type(&self, _: &Compiler, _: Span) -> TypeResult {
        Ok(void().into())
    }
}
