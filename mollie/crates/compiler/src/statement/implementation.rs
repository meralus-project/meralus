use indexmap::map::Entry;
use mollie_parser::{Impl, ImplFunction};
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ComplexType, Function, FunctionType, ObjectValue, Type, TypeKind, TypeVariant, VTable, Value, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile<Value> for Positioned<ImplFunction> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult<Value> {
        let args = usize::from(self.value.this.is_some()) + self.value.args.len();

        for arg in &self.value.args {
            let ty = arg.value.ty.get_type(compiler)?;

            compiler.var(&arg.value.name.value.0, ty.variant);
        }

        let chunk = compiler.compile(chunk, self.value.body)?;

        for arg in self.value.args {
            compiler.remove_var(&arg.value.name.value.0);
        }

        Ok(Value::object(ObjectValue::Function(Function { args, body: chunk })))
    }
}

impl GetType for ImplFunction {
    fn get_type(&self, compiler: &Compiler, span: Span) -> TypeResult {
        let mut args = Vec::new();

        if self.this.is_some() {
            args.push(compiler.get_type("self"));
        }

        for arg in &self.args {
            args.push(arg.value.ty.get_type(compiler)?);
        }

        let returns = self.returns.as_ref().map_or_else(|| Ok(void().into()), |returns| returns.get_type(compiler))?;

        Ok(Type {
            variant: TypeVariant::complex(ComplexType::Function(FunctionType {
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
        let ty = self.value.target.get_type(compiler)?;
        let mut trait_index = None;

        if let Some(trait_name) = self.value.trait_name {
            let r#trait = compiler.traits.get_index_of(&trait_name.value.0).ok_or_else(|| TypeError::NotFound {
                ty: Some(TypeKind::Trait),
                name: trait_name.value.0.clone(),
            })?;

            trait_index.replace(r#trait);

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

                        if !ty.variant.same_as(&expected.variant) {
                            return Err(TypeError::InvalidArgumentType {
                                got: ty.kind(),
                                expected: expected.kind(),
                            }
                            .into());
                        }
                    }
                } else {
                    return Err(TypeError::TraitFunctionNotFound {
                        trait_name: trait_name.value.0,
                        name: function.name.clone(),
                    }
                    .into());
                }
            }

            for function in &self.value.functions.value {
                if !compiler.traits[r#trait].functions.iter().any(|func| func.name == function.value.name.value.0) {
                    return Err(TypeError::UnknownTraitFunction {
                        trait_name: trait_name.value.0,
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

                if have_self {
                    compiler.var("self", ty.variant.clone());
                }

                let ty = function.get_type(compiler)?;
                let value = compiler.compile(chunk, function)?;

                if have_self {
                    compiler.remove_var("self");
                }

                Ok((name, (ty, trait_index, value)))
            })
            .collect::<CompileResult<_>>()?;

        match compiler.vtables.entry(ty.variant) {
            Entry::Occupied(mut entry) => entry.get_mut().functions.extend(functions),
            Entry::Vacant(entry) => {
                entry.insert(VTable { functions });
            }
        }

        Ok(())
    }
}

impl GetType for Impl {
    fn get_type(&self, _: &Compiler, _: Span) -> TypeResult {
        Ok(void().into())
    }
}
