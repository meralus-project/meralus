use mollie_parser::{Literal, Number as LiteralNumber};
use mollie_shared::{Positioned, Span};
use mollie_vm::{Chunk, ObjectValue, Value, boolean, float, integer, string, void};

use crate::{Compile, CompileResult, Compiler, GetType, TypeResult};

impl Compile for Positioned<Literal> {
    fn compile(self, chunk: &mut Chunk, _: &mut Compiler) -> CompileResult {
        use Literal::{SizeUnit, Number, Boolean, String};
        use LiteralNumber::{I64, F32};

        let constant: usize = match self.value {
            SizeUnit(..) => todo!(),
            Number(number) => match number {
                I64(value) => chunk.constant(Value::Integer(value)),
                F32(value) => chunk.constant(Value::Float(value)),
            },
            Boolean(value) => chunk.constant(Value::Boolean(value)),
            String(value) => chunk.constant(Value::object(ObjectValue::String(value))),
        };

        chunk.load_const(constant);

        Ok(())
    }
}

impl GetType for Literal {
    fn get_type(&self, _: &Compiler, _: Span) -> TypeResult {
        use Literal::{SizeUnit, Number, Boolean, String};
        use LiteralNumber::{I64, F32};

        Ok(match self {
            SizeUnit(..) => void(),
            Number(I64(_)) => integer(),
            Number(F32(_)) => float(),
            Boolean(_) => boolean(),
            String(_) => string(),
        }
        .into())
    }
}
