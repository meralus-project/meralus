use mollie_parser::{IndexExpr, IndexTarget};
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ComplexType, ComponentChildren, FunctionType, PrimitiveType, Type, TypeKind, TypeVariant, array_of, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile for Positioned<IndexExpr> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let ty = self.value.target.get_type(compiler)?;

        let assign = compiler.assign.take();
        let returns_value = assign.is_some();

        match self.value.index.value {
            IndexTarget::Named(index) => {
                if let Some((vtable, trait_index, function)) = compiler.find_vtable_function_index(&ty.variant, &index.0) {
                    if compiler.vtables[vtable][&trait_index][function]
                        .0
                        .variant
                        .as_function()
                        .is_some_and(|f| f.have_self)
                    {
                        println!("{:#?}", compiler.vtables[vtable]);
                        chunk.get_type_function(vtable, trait_index, function);
                        compiler.compile(chunk, *self.value.target)?;
                    } else {
                        return Err(TypeError::FunctionNotFound {
                            ty: Box::new(ty.variant.kind()),
                            ty_name: None,
                            function: index.0,
                        }
                        .into());
                    }
                } else if let Some(component) = ty.variant.as_component() {
                    compiler.compile(chunk, *self.value.target)?;

                    if let Some(pos) = component.properties.iter().position(|(name, ..)| name == &index.0) {
                        if let Some(assign) = assign {
                            compiler.compile(chunk, assign)?;

                            chunk.set_property(pos);
                        } else {
                            chunk.get_property(pos);
                        }
                    } else if index.0 == "children" && !matches!(component.children, ComponentChildren::None) {
                        if let Some(assign) = assign {
                            compiler.compile(chunk, assign)?;

                            chunk.set_property(component.properties.len());
                        } else {
                            chunk.get_property(component.properties.len());
                        }
                    } else {
                        return Err(TypeError::PropertyNotFound {
                            ty: Box::new(TypeKind::Component),
                            ty_name: None,
                            property: index.0,
                        }
                        .into());
                    }
                } else if let Some(structure) = ty.variant.as_struct() {
                    compiler.compile(chunk, *self.value.target)?;

                    if let Some(pos) = structure.properties.iter().position(|(name, ..)| name == &index.0) {
                        if let Some(assign) = assign {
                            compiler.compile(chunk, assign)?;

                            chunk.set_property(pos);
                        } else {
                            chunk.get_property(pos);
                        }
                    } else {
                        return Err(TypeError::PropertyNotFound {
                            ty: Box::new(TypeKind::Struct),
                            ty_name: None,
                            property: index.0,
                        }
                        .into());
                    }
                } else if let Some((_, trait_index)) = ty.variant.as_trait_instance() {
                    let function = compiler.traits[trait_index].functions.iter().position(|f| f.name == index.0).unwrap();

                    compiler.compile(chunk, *self.value.target)?;
                    chunk.get_type_function2(Some(trait_index), function);
                    // compiler.compile(chunk, *self.value.target)?;
                }
            }
            IndexTarget::Expression(expression) => {
                if ty.variant.as_array().is_some() {
                    compiler.compile(chunk, *self.value.target)?;
                    compiler.compile(chunk, self.value.index.span.wrap(*expression))?;

                    if let Some(assign) = assign {
                        compiler.compile(chunk, assign)?;

                        chunk.set_array_element();
                    } else {
                        chunk.get_array_element();
                    }
                }
            }
        }

        Ok(returns_value)
    }
}

impl GetType for IndexExpr {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        if compiler.assign.is_some() {
            return Ok(void().into());
        }

        let target = self.target.get_type(compiler)?;
        let mut result = match &self.index.value {
            IndexTarget::Named(property_name) => {
                if let Some((ty, ..)) = compiler.find_vtable_function(&target.variant, &property_name.0) {
                    return Ok(Type {
                        variant: ty.variant.clone(),
                        applied_generics: target.applied_generics,
                        declared_at: ty.declared_at,
                    });
                } else {
                    match target.variant {
                        TypeVariant::Generic(_) => todo!(),
                        TypeVariant::Primitive(_) => unimplemented!("primitive types doesn't have properties"),
                        TypeVariant::Complex(ref complex_type) => match &**complex_type {
                            ComplexType::Component(component) => {
                                if property_name.0 == "children" {
                                    return Ok(Type {
                                        variant: match component.children {
                                            mollie_vm::ComponentChildren::None => {
                                                return Err(TypeError::PropertyNotFound {
                                                    ty: Box::new(TypeKind::Component),
                                                    ty_name: None,
                                                    property: property_name.0.clone(),
                                                });
                                            }
                                            mollie_vm::ComponentChildren::Single => TypeVariant::Primitive(PrimitiveType::Component),
                                            mollie_vm::ComponentChildren::MaybeSingle => TypeVariant::complex(ComplexType::OneOf(vec![
                                                TypeVariant::Primitive(PrimitiveType::Component).into(),
                                                TypeVariant::Primitive(PrimitiveType::Null).into(),
                                            ])),
                                            mollie_vm::ComponentChildren::Multiple(s) => array_of(TypeVariant::Primitive(PrimitiveType::Component), s),
                                            mollie_vm::ComponentChildren::MaybeMultiple(s) => TypeVariant::complex(ComplexType::OneOf(vec![
                                                array_of(TypeVariant::Primitive(PrimitiveType::Component), s).into(),
                                                TypeVariant::Primitive(PrimitiveType::Null).into(),
                                            ])),
                                        },
                                        applied_generics: if target.applied_generics.is_empty() {
                                            vec![TypeVariant::Primitive(PrimitiveType::Component).into()]
                                        } else {
                                            target.applied_generics
                                        },
                                        declared_at: None,
                                    });
                                } else {
                                    component
                                        .properties
                                        .iter()
                                        .find(|(name, ..)| name == &property_name.0)
                                        .map(|(.., v)| v.clone().resolve_type(&target.applied_generics))
                                        .ok_or_else(|| TypeError::PropertyNotFound {
                                            ty: Box::new(TypeKind::Component),
                                            ty_name: None,
                                            property: property_name.0.clone(),
                                        })
                                }
                            }
                            ComplexType::Struct(structure) => structure
                                .properties
                                .iter()
                                .find(|(name, _)| name == &property_name.0)
                                .map(|(.., v)| v.clone().resolve_type(&target.applied_generics))
                                .ok_or_else(|| TypeError::PropertyNotFound {
                                    ty: Box::new(TypeKind::Struct),
                                    ty_name: None,
                                    property: property_name.0.clone(),
                                }),
                            ComplexType::TraitInstance(ty, trait_index) => compiler.traits[*trait_index]
                                .functions
                                .iter()
                                .find(|f| f.name == property_name.0)
                                .map(|f| {
                                    TypeVariant::complex(ComplexType::Function(FunctionType {
                                        is_native: false,
                                        have_self: f.this,
                                        args: {
                                            let mut args = vec![ty.clone()];

                                            args.extend(f.args.clone());

                                            args
                                        },
                                        returns: Box::new(f.returns.clone()),
                                    }))
                                    .into()
                                })
                                .ok_or_else(|| TypeError::PropertyNotFound {
                                    ty: Box::new(TypeKind::Struct),
                                    ty_name: None,
                                    property: property_name.0.clone(),
                                }),
                            _ => unimplemented!(
                                "{} cannot be indexed by {}",
                                target.clone().resolve_type(&target.applied_generics),
                                property_name.0
                            ),
                        },
                    }
                }
            }
            IndexTarget::Expression(expression) => {
                let index = expression.get_type(compiler, self.index.span)?;

                if !index.variant.same_as(&TypeVariant::Primitive(PrimitiveType::Integer), &target.applied_generics) {
                    return Err(TypeError::Unexpected {
                        got: Box::new(index.kind()),
                        expected: Box::new(TypeVariant::Primitive(PrimitiveType::Integer).kind().into()),
                    });
                }

                match target.variant {
                    TypeVariant::Generic(_) => todo!(),
                    TypeVariant::Primitive(_) => unimplemented!("primitive types doesn't have properties"),
                    TypeVariant::Complex(complex_type) => match &*complex_type {
                        ComplexType::Array(array) => Ok(array.element.clone()),
                        _ => unimplemented!("functions cannot be indexed"),
                    },
                }
            }
        }?;

        result.applied_generics.extend(target.applied_generics.into_iter());

        Ok(result)
    }
}
