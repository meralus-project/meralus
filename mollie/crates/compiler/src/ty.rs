use mollie_parser::{PrimitiveType, Type};
use mollie_shared::Span;
use mollie_vm::{ArrayType, ComplexType, PrimitiveType as PrimitiveVmType, TypeVariant, boolean, float, integer, string, void};

use crate::{Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl GetType for Type {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        use PrimitiveType::{Integer, Float, Boolean, String, Component, Void};
        use Type::{Primitive, Array, Custom};

        Ok(match self {
            Primitive(Integer) => integer().into(),
            Primitive(Float) => float().into(),
            Primitive(Boolean) => boolean().into(),
            Primitive(String) => string().into(),
            Primitive(Component) => TypeVariant::Primitive(PrimitiveVmType::Component).into(),
            Primitive(Void) => void().into(),
            Array(ty, size) => TypeVariant::complex(ComplexType::Array(ArrayType {
                element: ty.get_type(compiler)?,
                size: size.map(|v| v.value),
            }))
            .into(),
            Custom(name) => compiler.types.get(&name.0).cloned().ok_or_else(|| TypeError::NotFound {
                ty: None,
                name: name.0.clone(),
            })?,
        })
    }
}
