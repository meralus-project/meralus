use std::{
    cell::RefCell,
    fmt::{self, Write},
    ops::{Add, Div, Mul, Neg, Sub},
    rc::Rc,
    sync::Arc,
};

use mollie_shared::pretty_fmt::{PrettyFmt, indent_down, indent_up};
use serde::{Deserialize, Serialize};

use crate::{Chunk, Type, Vm, array_of, boolean, float, integer, null, string};

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "kebab-case")]
pub enum ObjectValue {
    String(String),
    Array(Vec<Value>),
    Component(Component),
    Struct(Struct),
    Function(Function),
    Enum(Enum),
    #[serde(skip)]
    NativeFunc(Arc<dyn Fn(&mut Vm, Vec<Value>) -> Option<Value>>),
}

impl ObjectValue {
    pub fn native_func<T: Fn(&mut Vm, Vec<Value>) -> Option<Value> + 'static>(func: T) -> Self {
        Self::NativeFunc(Arc::new(func))
    }
}

impl fmt::Debug for ObjectValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(arg0) => f.debug_tuple("String").field(arg0).finish(),
            Self::Array(arg0) => f.debug_tuple("Array").field(arg0).finish(),
            Self::Component(arg0) => f.debug_tuple("Component").field(arg0).finish(),
            Self::Struct(arg0) => f.debug_tuple("Struct").field(arg0).finish(),
            Self::Enum(arg0) => f.debug_tuple("Enym").field(arg0).finish(),
            Self::Function(arg0) => f.debug_tuple("Function").field(arg0).finish(),
            Self::NativeFunc(arg0) => f.debug_tuple("NativeFunc").field(&format!("{:p}", arg0)).finish(),
        }
    }
}

impl PartialEq for ObjectValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            (Self::Array(l0), Self::Array(r0)) => l0 == r0,
            (Self::Component(l0), Self::Component(r0)) => l0 == r0,
            (Self::Struct(l0), Self::Struct(r0)) => l0 == r0,
            (Self::Function(l0), Self::Function(r0)) => l0 == r0,
            (Self::NativeFunc(l0), Self::NativeFunc(r0)) => Arc::ptr_eq(l0, r0),
            _ => false,
        }
    }
}

impl PartialOrd for ObjectValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::String(l0), Self::String(r0)) => l0.partial_cmp(r0),
            (Self::Array(l0), Self::Array(r0)) => l0.partial_cmp(r0),
            (Self::Component(l0), Self::Component(r0)) => l0.partial_cmp(r0),
            (Self::Struct(l0), Self::Struct(r0)) => l0.partial_cmp(r0),
            (Self::Function(l0), Self::Function(r0)) => l0.partial_cmp(r0),
            (Self::NativeFunc(l0), Self::NativeFunc(r0)) => Arc::as_ptr(l0).cast::<()>().partial_cmp(&Arc::as_ptr(r0).cast::<()>()),
            _ => None,
        }
    }
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
            Self::Enum(enumeration) => {
                let ty = enumeration.ty.variant.as_enum().unwrap();
                let variant = &ty.variants[enumeration.variant];

                if let Some(properties) = &variant.1.properties {
                    if properties.len() == 0 {
                        write!(f, "enum({}) {{ }}", variant.0)
                    } else {
                        writeln!(f, "enum({}) {{", variant.0)?;

                        indent_down();

                        f.write_array_like(
                            properties.iter().zip(&enumeration.values).map(|((key, _), value)| format!("{key}: {value}")),
                            true,
                        )?;

                        indent_up();

                        f.write_char('\n')?;
                        f.write_indent()?;
                        f.write_char('}')
                    }
                } else {
                    write!(f, "enum({})", variant.0)
                }
            }
            Self::Component(component) => component.fmt(f),
            Self::Struct(structure) => structure.fmt(f),
            Self::Function(func) => func.fmt(f),
            Self::NativeFunc(_) => f.write_str("function"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, PartialOrd)]
pub struct Enum {
    pub ty: Type,
    pub variant: usize,
    pub values: Vec<Value>,
}

impl Enum {
    pub fn get_property<N: AsRef<str>, T: FromValue>(&self, name: N) -> Option<T> {
        let ty = self.ty.variant.as_enum()?;
        let property = ty.variants[self.variant]
            .1
            .properties
            .as_ref()?
            .iter()
            .position(|proprerty| proprerty.0 == name.as_ref())?;

        T::from_value(&self.values[property])
    }

    pub fn get_property_value<T: AsRef<str>>(&self, name: T) -> Option<&Value> {
        let ty = self.ty.variant.as_enum()?;
        let property = ty.variants[self.variant]
            .1
            .properties
            .as_ref()?
            .iter()
            .position(|proprerty| proprerty.0 == name.as_ref())?;

        Some(&self.values[property])
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, PartialOrd)]
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
        let ty = self.ty.variant.as_struct().unwrap();

        if ty.properties.len() == 0 {
            f.write_str("struct { }")
        } else {
            f.write_str("struct {\n")?;

            indent_down();

            f.write_array_like(ty.properties.iter().zip(&self.values).map(|((key, _), value)| format!("{key}: {value}")), true)?;

            indent_up();

            f.write_char('\n')?;
            f.write_indent()?;
            f.write_char('}')
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, PartialOrd)]
pub struct Component {
    pub ty: Type,
    pub values: Vec<Value>,
    pub children: Vec<Value>,
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ty = self.ty.variant.as_component().unwrap();

        if ty.properties.len() == 0 && self.children.is_empty() {
            f.write_str("component { }")
        } else {
            f.write_str("component {\n")?;

            indent_down();

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
}

#[derive(Debug, Deserialize, Serialize, PartialEq, PartialOrd)]
pub struct Function {
    pub have_self: bool,
    pub args: usize,
    pub body: Chunk,
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "function({}) {{", self.args)?;

        indent_down();

        f.write_array_like(&self.body.instructions, true)?;

        indent_up();

        f.write_char('\n')?;
        f.write_indent()?;
        f.write_char('}')
    }
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

#[derive(Debug, Deserialize, Serialize, PartialEq, PartialOrd, Clone)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all = "kebab-case")]
pub enum Value {
    Integer(i64),
    Float(f32),
    Boolean(bool),
    Object(Rc<RefCell<ObjectValue>>),
    Null,
}

impl Value {
    pub fn get_type(&self) -> Option<Type> {
        match self {
            Self::Integer(_) => Some(integer().into()),
            Self::Float(_) => Some(float().into()),
            Self::Boolean(_) => Some(boolean().into()),
            Self::Object(ref_cell) => match &*ref_cell.borrow() {
                ObjectValue::String(_) => Some(string().into()),
                ObjectValue::Array(values) => Some(array_of(values[0].get_type()?, Some(values.len())).into()),
                ObjectValue::Enum(enumeration) => Some(enumeration.ty.clone()),
                ObjectValue::Component(component) => Some(component.ty.clone()),
                ObjectValue::Struct(structure) => Some(structure.ty.clone()),
                ObjectValue::Function(_) | ObjectValue::NativeFunc(_) => None,
            },
            Self::Null => Some(null().into()),
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

impl Add for Value {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Integer(a), Self::Integer(b)) => Self::Integer(a + b),
            (Self::Float(a), Self::Float(b)) => Self::Float(a + b),
            (a, _) => a,
        }
    }
}

impl Sub for Value {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Integer(a), Self::Integer(b)) => Self::Integer(a - b),
            (Self::Float(a), Self::Float(b)) => Self::Float(a - b),
            (a, _) => a,
        }
    }
}

impl Mul for Value {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Integer(a), Self::Integer(b)) => Self::Integer(a * b),
            (Self::Float(a), Self::Float(b)) => Self::Float(a * b),
            (a, _) => a,
        }
    }
}

impl Div for Value {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Integer(a), Self::Integer(b)) => Self::Integer(a / b),
            (Self::Float(a), Self::Float(b)) => Self::Float(a / b),
            (a, _) => a,
        }
    }
}

impl Neg for Value {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Integer(value) => Self::Integer(-value),
            Self::Float(value) => Self::Float(-value),
            Self::Boolean(value) => Self::Boolean(!value),
            value => value,
        }
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
