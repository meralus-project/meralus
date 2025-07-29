use mollie_parser::Node;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ComponentChildren, TypeKind, Value};

use crate::{Compile, CompileError, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile for Positioned<Node> {
    fn compile(mut self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let ty = compiler.try_get_type(&self.value.name.value.0)?;

        if let Some(component) = ty.variant.as_component() {
            for prop in &self.value.properties {
                let got = prop.value.value.get_type(compiler)?;
                let (.., expected) =
                    component
                        .properties
                        .iter()
                        .find(|(name, ..)| name == &prop.value.name.value.0)
                        .ok_or_else(|| TypeError::PropertyNotFound {
                            ty: TypeKind::Component,
                            ty_name: Some(self.value.name.value.0.clone()),
                            property: prop.value.name.value.0.clone(),
                        })?;

                if !got.variant.same_as(&expected.variant) {
                    return Err(TypeError::Unexpected {
                        got: got.kind(),
                        expected: expected.kind(),
                    }
                    .into());
                }
            }

            let children = ComponentChildren::from(self.value.children.value.len());

            if !component.children.validate(&children) {
                return Err(TypeError::InvalidChildren {
                    got: children,
                    expected: component.children,
                    ty_name: self.value.name.value.0,
                }
                .into());
            }

            for (name, nullable, _) in &component.properties {
                let property = self
                    .value
                    .properties
                    .iter()
                    .position(|property| &property.value.name.value.0 == name)
                    .map(|index| self.value.properties.remove(index));

                if let Some(property) = property {
                    compiler.compile(chunk, property.value.value)?;
                } else if *nullable {
                    let constant = chunk.constant(Value::Null);

                    chunk.load_const(constant);
                }
            }

            let size = self.value.children.value.len();

            for node in self.value.children.value {
                compiler.compile(chunk, node)?;
            }

            let have_children = if size == 1 && matches!(component.children, ComponentChildren::Single | ComponentChildren::MaybeSingle) {
                true
            } else if matches!(component.children, ComponentChildren::Multiple(_) | ComponentChildren::MaybeMultiple(_)) {
                chunk.create_array(size);

                true
            } else {
                size != 0
            };


            let ty = compiler
                .types
                .get_index_of(&self.value.name.value.0)
                .ok_or(CompileError::VariableNotFound { name: self.value.name.value.0 })?;

            chunk.instantiate(ty, have_children);
        } else if let Some(structure) = ty.variant.as_struct() {
            for prop in &self.value.properties {
                let got = prop.value.value.get_type(compiler)?;
                let (.., expected) =
                    structure
                        .properties
                        .iter()
                        .find(|(name, ..)| name == &prop.value.name.value.0)
                        .ok_or_else(|| TypeError::PropertyNotFound {
                            ty: TypeKind::Struct,
                            ty_name: Some(self.value.name.value.0.clone()),
                            property: prop.value.name.value.0.clone(),
                        })?;

                if !got.variant.same_as(&expected.variant) {
                    return Err(TypeError::Unexpected {
                        got: got.kind(),
                        expected: expected.kind(),
                    }
                    .into());
                }
            }

            for (name, _) in &structure.properties {
                let property = self
                    .value
                    .properties
                    .iter()
                    .position(|property| &property.value.name.value.0 == name)
                    .map(|index| self.value.properties.remove(index));

                if let Some(property) = property {
                    compiler.compile(chunk, property.value.value)?;
                }/*  else if *nullable {
                    let constant = chunk.constant(Value::Null);

                    chunk.load_const(constant);
                } */
            }

            for node in self.value.children.value {
                compiler.compile(chunk, node)?;
            }

            let ty = compiler
                .types
                .get_index_of(&self.value.name.value.0)
                .ok_or(CompileError::VariableNotFound { name: self.value.name.value.0 })?;

            chunk.instantiate(ty, false);
        }

        Ok(())
    }
}

impl GetType for Node {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        let ty = compiler.types.get(&self.name.value.0).ok_or_else(|| TypeError::NotFound {
            ty: Some(TypeKind::OneOf(vec![TypeKind::Component, TypeKind::Struct])),
            name: self.name.value.0.clone(),
        })?;

        if ty.variant.is_component() || ty.variant.is_struct() {
            Ok(ty.clone())
        } else {
            Err(TypeError::Unexpected {
                got: ty.kind(),
                expected: TypeKind::Struct.into(),
            })
        }
    }
}
