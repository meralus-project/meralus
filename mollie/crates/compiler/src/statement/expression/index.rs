use mollie_parser::IndexExpr;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ComplexType, TypeKind, TypeVariant};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile for Positioned<IndexExpr> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let ty = self.value.target.get_type(compiler)?;

        if let Some((ty, function)) = compiler.get_vtable_index(&ty.variant).and_then(|ty| {
            compiler.vtables[ty]
                .functions
                .get_index_of(&self.value.index.value.0)
                .map(|function| (ty, function))
        }) {
            println!("index {ty} {function}");

            chunk.get_type_function(ty, function);
            compiler.compile(chunk, *self.value.target)?;
        } else if let Some(component) = ty.variant.as_component() {
            compiler.compile(chunk, *self.value.target)?;

            if let Some(pos) = component.properties.iter().position(|(name, ..)| name == &self.value.index.value.0) {
                chunk.get_property(pos);
            } else {
                return Err(TypeError::PropertyNotFound {
                    ty: TypeKind::Component,
                    ty_name: None,
                    property: self.value.index.value.0,
                }
                .into());
            }
        } else if let Some(structure) = ty.variant.as_struct() {
            compiler.compile(chunk, *self.value.target)?;

            if let Some(pos) = structure.properties.iter().position(|(name, ..)| name == &self.value.index.value.0) {
                chunk.get_property(pos);
            } else {
                return Err(TypeError::PropertyNotFound {
                    ty: TypeKind::Struct,
                    ty_name: None,
                    property: self.value.index.value.0,
                }
                .into());
            }
        }

        Ok(())
    }
}

impl GetType for IndexExpr {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        let target = self.target.get_type(compiler)?;

        if let Some((ty, ..)) = compiler.get_vtable(&target.variant).and_then(|vtable| {
            vtable.functions.get(&self.index.value.0)
        }) {
            Ok(ty.clone())
        } else {
            match target.variant {
                TypeVariant::Primitive(_) => unimplemented!("primitive types doesn't have properties"),
                TypeVariant::Complex(complex_type) => match &*complex_type {
                    ComplexType::Component(component) => component
                        .properties
                        .iter()
                        .find(|(name, ..)| name == &self.index.value.0)
                        .map(|(.., v)| v)
                        .cloned()
                        .ok_or_else(|| TypeError::PropertyNotFound {
                            ty: TypeKind::Component,
                            ty_name: None,
                            property: self.index.value.0.clone(),
                        }),
                    ComplexType::Struct(structure) => structure
                        .properties
                        .iter()
                        .find(|(name, _)| name == &self.index.value.0)
                        .map(|(_, v)| v)
                        .cloned()
                        .ok_or_else(|| TypeError::PropertyNotFound {
                            ty: TypeKind::Struct,
                            ty_name: None,
                            property: self.index.value.0.clone(),
                        }),
                    _ => unimplemented!("functions cannot be indexed"),
                },
            }
        }
    }
}
