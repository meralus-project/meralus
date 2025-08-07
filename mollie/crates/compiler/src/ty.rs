use mollie_parser::{PrimitiveType, Type};
use mollie_shared::Span;
use mollie_vm::{ArrayType, ComplexType, PrimitiveType as PrimitiveVmType, Type as VmType, TypeVariant, boolean, float, integer, null, string, void};

use crate::{Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl GetType for Type {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        use PrimitiveType::{Boolean, Component, Float, Integer, Null, String, Void};
        use Type::{Array, Custom, OneOf, Primitive};

        Ok(match self {
            Primitive(Integer) => integer().into(),
            Primitive(Float) => float().into(),
            Primitive(Boolean) => boolean().into(),
            Primitive(String) => string().into(),
            Primitive(Component) => TypeVariant::Primitive(PrimitiveVmType::Component).into(),
            Primitive(Void) => void().into(),
            Primitive(Null) => null().into(),
            Array(ty, size) => TypeVariant::complex(ComplexType::Array(ArrayType {
                element: ty.get_type(compiler)?,
                size: size.map(|v| v.value),
            }))
            .into(),
            OneOf(types) => TypeVariant::complex(ComplexType::OneOf(types.iter().map(|ty| ty.get_type(compiler)).collect::<TypeResult<_>>()?)).into(),
            Custom(name) => compiler
                .types
                .get(&name.name.value.0)
                .map(|ty| {
                    name.generics
                        .iter()
                        .map(|generic| generic.get_type(compiler))
                        .collect::<TypeResult<Vec<_>>>()
                        .map(|applied_generics| VmType {
                            variant: ty.variant.clone(),
                            applied_generics,
                            declared_at: ty.declared_at,
                        })
                })
                .ok_or_else(|| TypeError::NotFound {
                    ty: None,
                    name: name.name.value.0.clone(),
                })??,
        })
    }
}
