use indexmap::{IndexMap, map::Entry};
use mollie_parser::{Impl, ImplFunction};
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ComplexType, Function, FunctionType, ObjectValue, Type, TypeKind, TypeVariant, Value, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile<Value> for Positioned<ImplFunction> {
    fn compile(self, _: &mut Chunk, compiler: &mut Compiler) -> CompileResult<Value> {
        let args = usize::from(self.value.this.is_some()) + self.value.args.len();

        for arg in &self.value.args {
            let ty = arg.value.ty.get_type(compiler)?;

            compiler.var(&arg.value.name.value.0, ty.variant);
        }

        let mut chunk = Chunk::default();

        chunk.frame = compiler.current_frame_id();

        if compiler.compile(&mut chunk, self.value.body)? {
            chunk.ret();
        }

        chunk.halt();

        for arg in self.value.args {
            compiler.remove_var(&arg.value.name.value.0);
        }

        Ok(Value::object(ObjectValue::Function(Function {
            have_self: self.value.this.is_some(),
            args,
            body: chunk,
        })))
    }
}

impl GetType for ImplFunction {
    fn get_type(&self, compiler: &Compiler, span: Span) -> TypeResult {
        let mut args = Vec::new();

        if self.this.is_some() {
            args.push(compiler.get_local_type("self")?);
        }

        for arg in &self.args {
            args.push(arg.value.ty.get_type(compiler)?);
        }

        let returns = self.returns.as_ref().map_or_else(|| Ok(void().into()), |returns| returns.get_type(compiler))?;

        Ok(Type {
            applied_generics: Vec::new(),
            variant: TypeVariant::complex(ComplexType::Function(FunctionType {
                is_native: false,
                have_self: self.this.is_some(),
                args,
                returns: Box::new(returns),
            })),
            declared_at: Some(span),
        })
    }
}

impl Compile for Positioned<Impl> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        for (index, name) in self.value.generics.iter().enumerate() {
            compiler.add_type(&name.value.0, TypeVariant::Generic(index));
        }

        let ty = self.value.target.get_type(compiler)?;
        let mut trait_index = None;

        if let Some(trait_name) = self.value.trait_name {
            let r#trait = compiler
                .traits
                .get_index_of(&trait_name.value.name.value.0)
                .ok_or_else(|| TypeError::NotFound {
                    ty: Some(Box::new(TypeKind::Trait)),
                    name: trait_name.value.name.value.0.clone(),
                })?;

            trait_index.replace(r#trait);

            if !trait_name.value.generics.is_empty() {
                compiler.generics = trait_name.value.generics.iter().map(|g| g.get_type(compiler)).collect::<TypeResult<Vec<_>>>()?;
            }

            for function in &compiler.traits[r#trait].functions {
                if let Some(func) = self.value.functions.value.iter().find(|func| func.value.name.value.0 == function.name) {
                    if function.args.len() != func.value.args.len() {
                        return Err(TypeError::FunctionDefinitionInvalid { name: function.name.clone() }.into());
                    }

                    if function.this && func.value.this.is_none() || !function.this && func.value.this.is_some() {
                        return Err(TypeError::FunctionDefinitionInvalid { name: function.name.clone() }.into());
                    }

                    for (got, expected) in func.value.args.iter().zip(&function.args) {
                        let ty = got.value.ty.get_type(compiler)?;

                        if !ty.variant.same_as(&expected.variant, &compiler.generics) {
                            return Err(TypeError::InvalidArgumentType {
                                got: Box::new(ty.resolved_kind(&compiler.generics)),
                                expected: Box::new(expected.resolved_kind(&compiler.generics)),
                            }
                            .into());
                        }
                    }

                    let returns = func.value.returns.as_ref().map_or_else(|| Ok(void().into()), |r| r.get_type(compiler))?;

                    if !returns.variant.same_as(&function.returns.variant, &compiler.generics) {
                        return Err(TypeError::Unexpected {
                            got: Box::new(returns.resolved_kind(&compiler.generics)),
                            expected: Box::new(function.returns.resolved_kind(&compiler.generics)),
                        }
                        .into());
                    }
                } else {
                    return Err(TypeError::TraitFunctionNotFound {
                        trait_name: trait_name.value.name.value.0,
                        name: function.name.clone(),
                    }
                    .into());
                }
            }

            if !trait_name.value.generics.is_empty() {
                compiler.generics = Vec::new();
            }

            for function in &self.value.functions.value {
                if !compiler.traits[r#trait].functions.iter().any(|func| func.name == function.value.name.value.0) {
                    return Err(TypeError::UnknownTraitFunction {
                        trait_name: trait_name.value.name.value.0,
                        name: function.value.name.value.0.clone(),
                    }
                    .into());
                }
            }
        }

        let functions = self
            .value
            .functions
            .value
            .into_iter()
            .map(|function| {
                let name = function.value.name.value.0.clone();
                let have_self = function.value.this.is_some();

                // chunk.push_frame();
                compiler.push_frame();

                if have_self {
                    compiler.var("self", ty.variant.clone());
                }

                let ty = function.get_type(compiler)?;
                let value = compiler.compile(chunk, function)?;

                if have_self {
                    compiler.remove_var("self");
                }

                compiler.pop_frame();
                // chunk.pop_frame();

                Ok((name, (ty, value)))
            })
            .collect::<CompileResult<_>>()?;

        for name in &self.value.generics {
            compiler.remove_type(&name.value.0);
        }

        if let Some(trait_index) = trait_index {
            match compiler.impls.entry(ty.variant.clone()) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push(trait_index);
                }
                Entry::Vacant(entry) => {
                    entry.insert(vec![trait_index]);
                }
            }
        }

        match compiler.vtables.entry(ty.variant) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().insert(trait_index, functions);
            }
            Entry::Vacant(entry) => {
                entry.insert(IndexMap::from_iter([(trait_index, functions)]));
            }
        }

        Ok(false)
    }
}

impl GetType for Impl {
    fn get_type(&self, _: &Compiler, _: Span) -> TypeResult {
        Ok(void().into())
    }
}
