use indexmap::IndexMap;
use mollie_parser::Node;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ComponentChildren, Type, TypeKind, TypeVariant, Value};

use crate::{Compile, CompileError, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile for Positioned<Node> {
    fn compile(mut self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let ty = self.get_type(compiler)?;

        if let Some(component) = ty.variant.as_component() {
            for prop in &self.value.properties {
                let got = prop.value.value.get_type(compiler)?;
                let (.., expected) =
                    component
                        .properties
                        .iter()
                        .find(|(name, ..)| name == &prop.value.name.value.0)
                        .ok_or_else(|| TypeError::PropertyNotFound {
                            ty: Box::new(TypeKind::Component),
                            ty_name: Some(self.value.name.value.name.value.0.clone()),
                            property: prop.value.name.value.0.clone(),
                        })?;

                if !got.variant.same_as(&expected.variant, &compiler.generics) {
                    return Err(TypeError::Unexpected {
                        got: Box::new(got.kind()),
                        expected: Box::new(expected.kind()),
                    }
                    .into());
                }
            }

            let children = ComponentChildren::from(self.value.children.value.len());

            if !component.children.validate(&children) {
                return Err(TypeError::InvalidChildren {
                    got: children,
                    expected: component.children,
                    ty_name: self.value.name.value.name.value.0,
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
                .get_index_of(&self.value.name.value.name.value.0)
                .ok_or(CompileError::VariableNotFound {
                    name: self.value.name.value.name.value.0,
                })?;

            chunk.instantiate(ty, have_children);
        } else if let Some(structure) = ty.variant.as_struct() {
            for (name, _) in &structure.properties {
                let property = self
                    .value
                    .properties
                    .iter()
                    .position(|property| &property.value.name.value.0 == name)
                    .map(|index| self.value.properties.remove(index));

                if let Some(property) = property {
                    compiler.compile(chunk, property.value.value)?;
                }
            }

            for node in self.value.children.value {
                compiler.compile(chunk, node)?;
            }

            compiler.generics = Vec::new();

            let ty = compiler
                .types
                .get_index_of(&self.value.name.value.name.value.0)
                .ok_or(CompileError::VariableNotFound {
                    name: self.value.name.value.name.value.0,
                })?;

            chunk.instantiate(ty, false);
        }

        Ok(true)
    }
}

impl GetType for Node {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        let ty = compiler.types.get(&self.name.value.name.value.0).ok_or_else(|| TypeError::NotFound {
            ty: Some(Box::new(TypeKind::OneOf(vec![TypeKind::Component, TypeKind::Struct]))),
            name: self.name.value.name.value.0.clone(),
        })?;

        if let Some(structure) = ty.variant.as_struct() {
            let applied_generics = self
                .name
                .value
                .generics
                .iter()
                .map(|ty| ty.get_type(compiler))
                .collect::<TypeResult<Vec<_>>>()?;

            let mut resolved_generics = IndexMap::new();

            for prop in &self.properties {
                let got = prop.value.value.get_type(compiler)?;
                let (.., expected) =
                    structure
                        .properties
                        .iter()
                        .find(|(name, ..)| name == &prop.value.name.value.0)
                        .ok_or_else(|| TypeError::PropertyNotFound {
                            ty: Box::new(TypeKind::Struct),
                            ty_name: Some(self.name.value.name.value.0.clone()),
                            property: prop.value.name.value.0.clone(),
                        })?;

                if let TypeVariant::Generic(position) = expected.variant
                    && position >= applied_generics.len()
                {
                    resolved_generics.insert(position, got);
                } else if !got.variant.same_as(&expected.variant, &applied_generics) {
                    return Err(TypeError::Unexpected {
                        got: Box::new(got.kind()),
                        expected: Box::new(expected.kind()),
                    }
                    .into());
                }
            }

            if !resolved_generics.is_empty() {
                resolved_generics.sort_unstable_keys();

                Ok(Type {
                    variant: ty.variant.clone(),
                    applied_generics: resolved_generics.into_values().collect(),
                    declared_at: ty.declared_at,
                })
            } else {
                Ok(Type {
                    variant: ty.variant.clone(),
                    applied_generics,
                    declared_at: ty.declared_at,
                })
            }
        } else if ty.variant.is_component() {
            Ok(Type {
                variant: ty.variant.clone(),
                applied_generics: self
                    .name
                    .value
                    .generics
                    .iter()
                    .map(|ty| ty.get_type(compiler))
                    .collect::<TypeResult<Vec<_>>>()?,
                declared_at: ty.declared_at,
            })
        } else {
            Err(TypeError::Unexpected {
                got: Box::new(ty.kind()),
                expected: Box::new(TypeKind::Struct.into()),
            })
        }
    }
}
