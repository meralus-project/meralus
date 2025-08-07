use mollie_parser::ComponentDecl;
use mollie_shared::{Positioned, Span};
use mollie_vm::{ArrayType, Chunk, ComplexType, ComponentChildren, ComponentType, PrimitiveType, Type, TypeKind, TypeVariant, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile for Positioned<ComponentDecl> {
    fn compile(self, _: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let mut properties = Vec::new();
        let mut children = ComponentChildren::None;

        for property in &self.value.properties {
            let nullable = property.value.nullable.is_some();
            let name = &property.value.name.value.0;
            let ty = property.value.ty.get_type(compiler)?;

            if name == "children" {
                if matches!(ty.variant, TypeVariant::Primitive(PrimitiveType::Component)) {
                    children = if nullable {
                        ComponentChildren::MaybeSingle
                    } else {
                        ComponentChildren::Single
                    };
                } else if let Some(ArrayType {
                    element:
                        Type {
                            variant: TypeVariant::Primitive(PrimitiveType::Component),
                            ..
                        },
                    size,
                }) = ty.variant.as_array()
                {
                    children = if nullable {
                        ComponentChildren::MaybeMultiple(*size)
                    } else {
                        ComponentChildren::Multiple(*size)
                    };
                } else {
                    return Err(TypeError::Unexpected {
                        got: Box::new(ty.kind()),
                        expected: Box::new(TypeKind::Component.into()),
                    }
                    .into());
                }
            } else {
                properties.push((name.clone(), nullable, ty));
            }
        }

        let ty = Type {
            applied_generics: Vec::new(),
            variant: TypeVariant::complex(ComplexType::Component(ComponentType { properties, children })),
            declared_at: Some(self.span),
        };

        compiler.types.insert(self.value.name.value.0, ty);

        // compiler.var(self.value.name.value.0, ty.variant);

        Ok(false)
    }
}

impl GetType for ComponentDecl {
    fn get_type(&self, _: &Compiler, _: Span) -> TypeResult {
        Ok(void().into())
    }
}
