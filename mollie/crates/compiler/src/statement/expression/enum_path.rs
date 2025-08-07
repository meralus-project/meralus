use indexmap::IndexMap;
use mollie_parser::EnumPathExpr;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, Enum, ObjectValue, Type, TypeKind, TypeVariant, Value};

use crate::{Compile, CompileError, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile for Positioned<EnumPathExpr> {
    fn compile(mut self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let ty = self.get_type(compiler)?;

        if let Some(enumeration) = ty.variant.as_enum() {
            let variant = enumeration.variants.iter().position(|v| v.0 == self.value.index.value.name.value.0).unwrap();

            if let Some(properties) = &enumeration.variants[variant].1.properties {
                compiler.generics = ty.applied_generics;

                for (name, _) in properties {
                    let property = self
                        .value
                        .properties
                        .as_ref()
                        .unwrap()
                        .value
                        .iter()
                        .position(|prop| &prop.value.name.value.0 == name)
                        .map(|index| self.value.properties.as_mut().unwrap().value.remove(index));

                    let property = property.unwrap();

                    compiler.compile(chunk, property.value.value)?;
                }

                compiler.generics = Vec::new();

                let ty = compiler.types.get_index_of(&self.value.target.value.0).ok_or(CompileError::VariableNotFound {
                    name: self.value.target.value.0,
                })?;

                chunk.instantiate_variant(ty, variant);
            } else {
                let constant = chunk.constant(Value::object(ObjectValue::Enum(Enum {
                    ty,
                    variant,
                    values: Vec::new(),
                })));

                chunk.load_const(constant);
            }
        }

        Ok(true)
    }
}

impl GetType for EnumPathExpr {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        let ty = self.target.get_type(compiler)?;
        let applied_generics = self
            .index
            .value
            .generics
            .iter()
            .map(|ty| ty.get_type(compiler))
            .collect::<TypeResult<Vec<_>>>()?;

        if let Some(enumeration) = ty.variant.as_enum() {
            let mut resolved_generics = IndexMap::new();

            if let Some((_, variant)) = enumeration.variants.iter().find(|variant| variant.0 == self.target.value.0) {
                if let (Some(properties), Some(expected_properties)) = (&self.properties, &variant.properties) {
                    for prop in &properties.value {
                        let got = prop.value.value.get_type(compiler)?;
                        let (.., expected) =
                            expected_properties
                                .iter()
                                .find(|(name, ..)| name == &prop.value.name.value.0)
                                .ok_or_else(|| TypeError::PropertyNotFound {
                                    ty: Box::new(TypeKind::Enum),
                                    ty_name: Some(self.index.value.name.value.0.clone()),
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
                }
            }

            if !resolved_generics.is_empty() {
                resolved_generics.sort_unstable_keys();

                Ok(Type {
                    variant: ty.variant,
                    applied_generics: resolved_generics.into_values().collect(),
                    declared_at: ty.declared_at,
                })
            } else {
                Ok(Type {
                    variant: ty.variant,
                    applied_generics,
                    declared_at: ty.declared_at,
                })
            }
        } else {
            Ok(Type {
                variant: ty.variant,
                applied_generics,
                declared_at: ty.declared_at,
            })
        }
    }
}
