use mollie_parser::FunctionCall;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, TypeKind};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeError, TypeResult};

impl Compile for Positioned<FunctionCall> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let ty = self.value.function.get_type(compiler)?;

        if let Some(function) = ty.variant.as_function() {
            if function.have_self {
                println!("{function:#?}");
                
                if self.value.args.value.len() != function.args.len() - 1 {
                    return Err(TypeError::InvalidArguments {
                        got: self.value.args.value.len(),
                        expected: function.args.len() - 1,
                    }
                    .into());
                }


                compiler.compile(chunk, *self.value.function)?;

                for (arg, expected) in self.value.args.value.into_iter().zip(function.args.iter().skip(1)) {
                    let got = arg.get_type(compiler)?;

                    if !got.variant.same_as(&expected.variant) {
                        return Err(TypeError::Unexpected {
                            got: got.kind(),
                            expected: expected.kind(),
                        }
                        .into());
                    }

                    compiler.compile(chunk, arg)?;
                }
            } else {
                if self.value.args.value.len() != function.args.len() {
                    return Err(TypeError::InvalidArguments {
                        got: self.value.args.value.len(),
                        expected: function.args.len(),
                    }
                    .into());
                }

                compiler.compile(chunk, *self.value.function)?;

                for (arg, expected) in self.value.args.value.into_iter().zip(function.args.iter()) {
                    let got = arg.get_type(compiler)?;

                    if !got.variant.same_as(&expected.variant) {
                        return Err(TypeError::Unexpected {
                            got: got.kind(),
                            expected: expected.kind(),
                        }
                        .into());
                    }

                    compiler.compile(chunk, arg)?;
                }
            }

            chunk.call(function.args.len());

            Ok(())
        } else {
            Err(TypeError::Unexpected {
                got: ty.kind(),
                expected: TypeKind::Function.into(),
            }
            .into())
        }
    }
}

impl GetType for FunctionCall {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        let ty = self.function.get_type(compiler)?;

        ty.variant.as_function().map_or_else(
            || {
                Err(TypeError::Unexpected {
                    got: ty.kind(),
                    expected: TypeKind::Function.into(),
                })
            },
            |function| Ok(*(function.returns.clone())),
        )
    }
}
