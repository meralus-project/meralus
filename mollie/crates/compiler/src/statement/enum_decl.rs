use mollie_parser::EnumDecl;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ComplexType, EnumType, EnumVariant, Type, TypeVariant, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<EnumDecl> {
    fn compile(self, _: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let mut variants = Vec::new();

        for (index, name) in self.value.name.value.generics.iter().enumerate() {
            compiler.add_type(&name.value.0, TypeVariant::Generic(index));
        }

        for variant in self.value.variants.value {
            let properties = if let Some(properties) = variant.value.properties {
                let mut props = Vec::with_capacity(properties.value.len());

                for property in properties.value {
                    props.push((property.value.name.value.0, property.value.ty.get_type(compiler)?));
                }

                Some(props)
            } else {
                None
            };

            variants.push((variant.value.name.value.0, EnumVariant { properties }));
        }

        for name in &self.value.name.value.generics {
            compiler.remove_type(&name.value.0);
        }

        let ty = Type {
            applied_generics: Vec::new(),
            variant: TypeVariant::complex(ComplexType::EnumType(EnumType {
                generics: self.value.name.value.generics.into_iter().map(|g| g.value.0).collect(),
                variants,
            })),
            declared_at: Some(self.span),
        };

        compiler.types.insert(self.value.name.value.name.value.0, ty);

        // compiler.var(self.value.name.value.0, ty.variant);

        Ok(false)
    }
}

impl GetType for EnumDecl {
    fn get_type(&self, _: &Compiler, _: Span) -> TypeResult {
        Ok(void().into())
    }
}
