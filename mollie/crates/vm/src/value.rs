use std::{
    cell::RefCell,
    fmt::{self, Write},
    rc::Rc,
};

use mollie_shared::pretty_fmt::{PrettyFmt, indent_down, indent_up};

use crate::{Chunk, Type, TypeVariant, Vm, array_of, boolean, float, integer, string};

#[derive(Debug, PartialEq)]
pub enum ObjectValue {
    String(String),
    Array(Vec<Value>),
    Component(Component),
    Struct(Struct),
    Function(Function),
    NativeFunc(fn(&mut Vm, Vec<Value>) -> Option<Value>),
}

impl ObjectValue {
    pub const fn as_struct(&self) -> Option<&Struct> {
        if let Self::Struct(structure) = self { Some(structure) } else { None }
    }
}

impl fmt::Display for ObjectValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(value) => fmt::Debug::fmt(value, f),
            Self::Array(values) => {
                f.write_char('[')?;
                f.write_array_like(values, false)?;
                f.write_char(']')
            }
            Self::Component(component) => component.fmt(f),
            Self::Struct(structure) => structure.fmt(f),
            Self::Function(_) | Self::NativeFunc(_) => f.write_str("function"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Struct {
    pub ty: Type,
    pub values: Vec<Value>,
}

impl Struct {
    pub fn get_property<N: AsRef<str>, T: FromValue>(&self, name: N) -> Option<T> {
        let ty = self.ty.variant.as_struct()?;
        let property = ty.properties.iter().position(|proprerty| proprerty.0 == name.as_ref())?;

        T::from_value(&self.values[property])
    }

    pub fn get_property_value<T: AsRef<str>>(&self, name: T) -> Option<&Value> {
        let ty = self.ty.variant.as_struct()?;
        let property = ty.properties.iter().position(|proprerty| proprerty.0 == name.as_ref())?;

        Some(&self.values[property])
    }
}

impl fmt::Display for Struct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("struct {\n")?;

        indent_down();

        let ty = self.ty.variant.as_struct().unwrap();

        f.write_array_like(ty.properties.iter().zip(&self.values).map(|((key, _), value)| format!("{key}: {value}")), true)?;

        indent_up();

        f.write_char('\n')?;
        f.write_indent()?;
        f.write_char('}')
    }
}

#[derive(Debug, PartialEq)]
pub struct Component {
    pub ty: Type,
    pub values: Vec<Value>,
    pub children: Vec<Value>,
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("component {\n")?;

        indent_down();

        let ty = self.ty.variant.as_component().unwrap();

        f.write_array_like(ty.properties.iter().zip(&self.values).map(|((key, ..), value)| format!("{key}: {value}")), true)?;

        indent_up();

        if !self.children.is_empty() {
            f.write_str(",\n\n")?;

            indent_down();

            f.write_array_like(&self.children, true)?;

            indent_up();
        }

        f.write_char('\n')?;
        f.write_indent()?;
        f.write_char('}')
    }
}

#[derive(Debug, PartialEq)]
pub struct Function {
    pub args: usize,
    pub body: Chunk,
}

pub trait FromValue {
    fn from_value(value: &Value) -> Option<Self>
    where
        Self: Sized;
}

macro_rules! impl_value_to_num {
    ($($num:ty),*) => {
        $(
            impl FromValue for $num {
                fn from_value(value: &Value) -> Option<Self>
                where
                    Self: Sized,
                {
                    value.as_integer()?.try_into().ok()
                }
            }
        )*
    };
}

impl_value_to_num![u8, u16, u32, u64, usize, i8, i16, i32, i64, isize];

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Integer(i64),
    Float(f32),
    Boolean(bool),
    Object(Rc<RefCell<ObjectValue>>),
    Null,
}

impl Value {
    pub fn get_type(&self) -> Option<TypeVariant> {
        match self {
            Self::Integer(_) => Some(integer()),
            Self::Float(_) => Some(float()),
            Self::Boolean(_) => Some(boolean()),
            Self::Object(ref_cell) => match &*ref_cell.borrow() {
                ObjectValue::String(_) => Some(string()),
                ObjectValue::Array(values) => Some(array_of(values[0].get_type()?, Some(values.len()))),
                ObjectValue::Component(component) => Some(component.ty.variant.clone()),
                ObjectValue::Struct(structure) => Some(structure.ty.variant.clone()),
                ObjectValue::Function(_) | ObjectValue::NativeFunc(_) => None,
                },
            Self::Null => None,
        }
    }

    pub fn to<T: FromValue>(&self) -> Option<T> {
        T::from_value(self)
    }

    pub fn to_integer(&self) -> i64 {
        if let Self::Integer(value) = self { *value } else { unreachable!() }
    }

    pub fn to_float(&self) -> f32 {
        if let Self::Float(value) = self { *value } else { unreachable!() }
    }

    pub const fn as_integer(&self) -> Option<i64> {
        if let Self::Integer(value) = self { Some(*value) } else { None }
    }

    pub const fn as_float(&self) -> Option<f32> {
        if let Self::Float(value) = self { Some(*value) } else { None }
    }

    pub const fn as_object(&self) -> Option<&Rc<RefCell<ObjectValue>>> {
        if let Self::Object(object) = self { Some(object) } else { None }
    }

    pub fn object(value: ObjectValue) -> Self {
        Self::Object(Rc::new(RefCell::new(value)))
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(value) => value.fmt(f),
            Self::Float(value) => value.fmt(f),
            Self::Boolean(value) => value.fmt(f),
            Self::Object(object) => object.borrow().fmt(f),
            Self::Null => f.write_str("null"),
        }
    }
}
