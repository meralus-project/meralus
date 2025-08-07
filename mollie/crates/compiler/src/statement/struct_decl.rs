use mollie_parser::StructDecl;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ComplexType, StructType, Type, TypeVariant, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<StructDecl> {
    fn compile(self, _: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let mut properties = Vec::new();

        for (index, name) in self.value.name.value.generics.iter().enumerate() {
            compiler.add_type(&name.value.0, TypeVariant::Generic(index));
        }

        for property in &self.value.properties.value {
            // let nullable = property.value.nullable.is_some();
            let name = &property.value.name.value.0;
            let ty = property.value.ty.get_type(compiler)?;

            properties.push((name.clone(), /* nullable, */ ty));
        }

        for name in &self.value.name.value.generics {
            compiler.remove_type(&name.value.0);
        }

        let ty = Type {
            applied_generics: Vec::new(),
            variant: TypeVariant::complex(ComplexType::Struct(StructType {
                generics: self.value.name.value.generics.into_iter().map(|g| g.value.0).collect(),
                properties,
            })),
            declared_at: Some(self.span),
        };

        compiler.types.insert(self.value.name.value.name.value.0, ty);

        // compiler.var(self.value.name.value.0, ty.variant);

        Ok(false)
    }
}

impl GetType for StructDecl {
    fn get_type(&self, _: &Compiler, _: Span) -> TypeResult {
        Ok(void().into())
    }
}
