use std::fmt::{self, Write};

use derive_more::Display;
use mollie_shared::pretty_fmt::{PrettyFmt, indent_down, indent_up};

use crate::Value;

#[derive(Debug, PartialEq, Default)]
pub struct Chunk {
    pub constants: Vec<Value>,
    pub instructions: Vec<Inst>,
}

impl fmt::Display for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("chunk {\n")?;

        indent_down();

        f.write_indent()?;
        f.write_str("constants: [")?;

        f.write_array_like(&self.constants, false)?;

        f.write_str("],\n")?;

        f.write_indent()?;
        f.write_str("instructions: [\n")?;

        indent_down();

        f.write_array_like(&self.instructions, true)?;

        indent_up();

        f.write_char('\n')?;
        f.write_indent()?;
        f.write_char(']')?;

        indent_up();

        f.write_char('\n')?;
        f.write_indent()?;
        f.write_char('}')
    }
}

impl Chunk {
    pub fn constant(&mut self, value: Value) -> usize {
        if let Some(index) = self.constants.iter().position(|constant| constant == &value) {
            index
        } else {
            let index = self.constants.len();

            self.constants.push(value);

            index
        }
    }

    pub fn pop(&mut self) {
        self.instructions.push(Inst::Pop);
    }

    pub fn load_const(&mut self, constant: usize) {
        self.instructions.push(Inst::LoadConst(constant));
    }

    pub fn set_local(&mut self, id: usize) {
        self.instructions.push(Inst::SetLocal(id));
    }

    pub fn get_local(&mut self, id: usize) {
        self.instructions.push(Inst::GetLocal(id));
    }

    pub fn call(&mut self, args: usize) {
        self.instructions.push(Inst::Call(args));
    }

    pub fn create_array(&mut self, size: usize) {
        self.instructions.push(Inst::CreateArray(size));
    }

    pub fn instantiate(&mut self, ty: usize, have_children: bool) {
        self.instructions.push(Inst::Instantiate(ty, have_children));
    }

    pub fn get_type_function(&mut self, ty: usize, function: usize) {
        self.instructions.push(Inst::GetTypeFunction(ty, function));
    }

    pub fn get_property(&mut self, pos: usize) {
        self.instructions.push(Inst::GetProperty(pos));
    }

    pub fn ret(&mut self) {
        self.instructions.push(Inst::Ret);
    }

    pub fn halt(&mut self) {
        self.instructions.push(Inst::Halt);
    }
}

#[derive(Display, Debug, PartialEq, Eq)]
pub enum Inst {
    #[display("pop   ")]
    Pop,
    #[display("ldc  {_0}")]
    LoadConst(usize),
    #[display("setl {_0}")]
    SetLocal(usize),
    #[display("getl {_0}")]
    GetLocal(usize),
    #[display("carr {_0}")]
    CreateArray(usize),
    #[display("inst {_0} {_1}")]
    Instantiate(usize, bool),
    #[display("getf {_0} {_1}")]
    GetTypeFunction(usize, usize),
    #[display("getp {_0}")]
    GetProperty(usize),
    #[display("call {_0}")]
    Call(usize),
    #[display("ret   ")]
    Ret,
    #[display("halt  ")]
    Halt,
}
