use mollie_parser::TraitDecl;
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, Trait, TraitFunc, void};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<TraitDecl> {
    fn compile(self, _: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        let functions = self
            .value
            .functions
            .value
            .into_iter()
            .map(|function| {
                Ok(TraitFunc {
                    name: function.value.name.value.0,
                    this: function.value.this.is_some(),
                    args: function
                        .value
                        .args
                        .into_iter()
                        .map(|arg| arg.value.ty.get_type(compiler))
                        .collect::<TypeResult<_>>()?,
                    returns: if let Some(returns) = function.value.returns {
                        returns.get_type(compiler)?
                    } else {
                        void().into()
                    },
                })
            })
            .collect::<CompileResult<_>>()?;

        compiler.traits.insert(self.value.name.value.0, Trait {
            functions,
            declared_at: Some(self.span),
        });

        Ok(())
    }
}

impl GetType for TraitDecl {
    fn get_type(&self, _: &Compiler, _: Span) -> TypeResult {
        Ok(void().into())
    }
}
