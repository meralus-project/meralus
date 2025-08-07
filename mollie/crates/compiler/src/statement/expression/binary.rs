use mollie_parser::BinaryExpression;
use mollie_shared::{Operator, Positioned, Span};
use mollie_vm::{Chunk, boolean};

use crate::{Compile, CompileResult, Compiler, GetPositionedType, GetType, TypeResult};

impl Compile for Positioned<BinaryExpression> {
    fn compile(self, chunk: &mut Chunk, compiler: &mut Compiler) -> CompileResult {
        if self.value.operator.value == Operator::Assign {
            compiler.assign.replace(*self.value.rhs);

            compiler.compile(chunk, *self.value.lhs)?;

            Ok(false)
        } else {
            compiler.compile(chunk, *self.value.lhs)?;
            compiler.compile(chunk, *self.value.rhs)?;

            match self.value.operator.value {
                Operator::Add => chunk.add(),
                Operator::Sub => chunk.sub(),
                Operator::Mul => chunk.mul(),
                Operator::Div => chunk.div(),
                Operator::Equal => chunk.equals(),
                Operator::NotEqual => {
                    chunk.equals();
                    chunk.negate();
                }
                Operator::LessThan => chunk.less_than(),
                Operator::GreaterThan => chunk.greater_than(),
                _ => unreachable!(),
            }

            Ok(true)
        }
    }
}

impl GetType for BinaryExpression {
    fn get_type(&self, compiler: &Compiler, _: Span) -> TypeResult {
        if matches!(self.operator.value, Operator::Equal | Operator::NotEqual) {
            Ok(boolean().into())
        } else {
            self.lhs.get_type(compiler)
        }
    }
}
