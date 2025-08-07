use std::{
    fmt::{self, Write},
    ops::{Index, IndexMut},
};

use derive_more::Display;
use mollie_shared::pretty_fmt::{PrettyFmt, indent_down, indent_up};
use serde::{Deserialize, Serialize};

use crate::Value;

#[derive(Debug, Deserialize, Serialize, PartialEq, PartialOrd, Default)]
pub struct Chunk {
    pub frame: usize,
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

        let linebreak = true;
        let last_index = self.instructions.len() - 1;

        for (i, item) in self.instructions.iter().enumerate() {
            if linebreak {
                f.write_indent()?;
            }

            item.fmt(f)?;

            if let Inst::LoadConst(constant) = item {
                write!(f, ", // {}", self.constants[*constant])?;

                if linebreak {
                    writeln!(f)?;
                }
            } else if let Inst::Jump(length) = item {
                write!(f, ", // jump to {}", self.instructions[(i as isize + *length) as usize])?;

                if linebreak {
                    writeln!(f)?;
                }
            } else if let Inst::JumpIfFalse(length) = item {
                write!(f, ", // possible jump to {}", self.instructions[i + *length])?;

                if linebreak {
                    writeln!(f)?;
                }
            } else if linebreak && i != last_index {
                f.write_str(",\n")?;
            } else {
                f.write_str(", ")?;
            }
        }

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

    #[must_use]
    pub const fn len(&self) -> usize {
        self.instructions.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    pub fn add(&mut self) {
        self.instructions.push(Inst::Add);
    }

    pub fn mul(&mut self) {
        self.instructions.push(Inst::Mul);
    }

    pub fn div(&mut self) {
        self.instructions.push(Inst::Div);
    }

    pub fn sub(&mut self) {
        self.instructions.push(Inst::Sub);
    }

    pub fn equals(&mut self) {
        self.instructions.push(Inst::Equals);
    }

    pub fn negate(&mut self) {
        self.instructions.push(Inst::Negate);
    }

    pub fn less_than(&mut self) {
        self.instructions.push(Inst::LessThan);
    }

    pub fn greater_than(&mut self) {
        self.instructions.push(Inst::GreaterThan);
    }

    pub fn pop(&mut self) {
        self.instructions.push(Inst::Pop);
    }

    pub fn push_frame(&mut self) {
        self.instructions.push(Inst::PushFrame);
    }

    pub fn pop_frame(&mut self) {
        self.instructions.push(Inst::PopFrame);
    }

    pub fn load_const(&mut self, constant: usize) {
        self.instructions.push(Inst::LoadConst(constant));
    }

    pub fn set_local(&mut self, id: usize) {
        self.instructions.push(Inst::SetLocal(id));
    }

    pub fn get_local(&mut self, frame: usize, id: usize) {
        self.instructions.push(Inst::GetLocal(frame, id));
    }

    pub fn jump(&mut self, count: isize) {
        self.instructions.push(Inst::Jump(count));
    }

    pub fn jump_if_false(&mut self, count: usize) {
        self.instructions.push(Inst::JumpIfFalse(count));
    }

    pub fn call(&mut self, args: usize) {
        self.instructions.push(Inst::Call(args));
    }

    pub fn create_array(&mut self, size: usize) {
        self.instructions.push(Inst::CreateArray(size));
    }

    pub fn copy(&mut self) {
        self.instructions.push(Inst::Copy);
    }

    pub fn impls(&mut self, trait_index: usize) {
        self.instructions.push(Inst::Impls(trait_index));
    }

    pub fn is_instance_of(&mut self, ty: usize, variant: usize) {
        self.instructions.push(Inst::IsInstanceOf(ty, variant));
    }

    pub fn instantiate(&mut self, ty: usize, have_children: bool) {
        self.instructions.push(Inst::Instantiate(ty, 0, have_children));
    }

    pub fn instantiate_variant(&mut self, ty: usize, variant: usize) {
        self.instructions.push(Inst::Instantiate(ty, variant, false));
    }

    pub fn get_type_function(&mut self, ty: usize, trait_index: Option<usize>, function: usize) {
        self.instructions.push(Inst::GetTypeFunction(ty, trait_index, function));
    }

    pub fn get_type_function2(&mut self, trait_index: Option<usize>, function: usize) {
        self.instructions.push(Inst::GetTypeFunction2(trait_index, function));
    }

    pub fn get_array_element(&mut self) {
        self.instructions.push(Inst::GetArrayElement);
    }

    pub fn set_array_element(&mut self) {
        self.instructions.push(Inst::SetArrayElement);
    }

    pub fn get_property(&mut self, pos: usize) {
        self.instructions.push(Inst::GetProperty(pos));
    }

    pub fn set_property(&mut self, pos: usize) {
        self.instructions.push(Inst::SetProperty(pos));
    }

    pub fn ret(&mut self) {
        self.instructions.push(Inst::Ret);
    }

    pub fn halt(&mut self) {
        self.instructions.push(Inst::Halt);
    }
}

impl Index<usize> for Chunk {
    type Output = Inst;

    fn index(&self, index: usize) -> &Self::Output {
        &self.instructions[index]
    }
}

impl IndexMut<usize> for Chunk {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.instructions[index]
    }
}

#[derive(Display, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(tag = "type", content = "args")]
#[serde(rename_all = "kebab-case")]
pub enum Inst {
    #[display("add   ")]
    Add,
    #[display("mul   ")]
    Mul,
    #[display("div   ")]
    Div,
    #[display("sub   ")]
    Sub,
    #[display("pop   ")]
    Pop,
    #[display("pshf  ")]
    PushFrame,
    #[display("popf  ")]
    PopFrame,
    #[display("copy  ")]
    Copy,
    #[display("ldc  {_0}")]
    LoadConst(usize),
    #[display("setl {_0}")]
    SetLocal(usize),
    #[display("getl {_0} {_1}")]
    GetLocal(usize, usize),
    #[display("carr {_0}")]
    CreateArray(usize),
    #[display("impl {_0}")]
    Impls(usize),
    #[display("iins {_0} {_1}")]
    IsInstanceOf(usize, usize),
    #[display("inst {_0} {_1} {_2}")]
    Instantiate(usize, usize, bool),
    #[display("getf {_0} {_1:?} {_2}")]
    GetTypeFunction(usize, Option<usize>, usize),
    #[display("gef2 {_0:?} {_1}")]
    GetTypeFunction2(Option<usize>, usize),
    #[display("jmp  {_0}")]
    Jump(isize),
    #[display("jmpf {_0}")]
    JumpIfFalse(usize),
    #[display("gete  ")]
    GetArrayElement,
    #[display("sete  ")]
    SetArrayElement,
    #[display("getp {_0}")]
    GetProperty(usize),
    #[display("setp {_0}")]
    SetProperty(usize),
    #[display("call {_0}")]
    Call(usize),
    #[display("eq    ")]
    Equals,
    #[display("neg   ")]
    Negate,
    #[display("lt    ")]
    LessThan,
    #[display("gt    ")]
    GreaterThan,
    #[display("ret   ")]
    Ret,
    #[display("halt  ")]
    Halt,
}
