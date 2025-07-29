mod array;
mod component;
mod function;
mod kind;
mod primitive;
mod structure;

use std::{fmt, sync::Arc};

use mollie_shared::{MaybePositioned, Span, SpanType};

pub use self::{
    array::ArrayType,
    component::{ComponentChildren, ComponentType},
    function::FunctionType,
    kind::TypeKind,
    primitive::PrimitiveType,
    structure::StructType,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitFunc {
    pub name: String,
    pub this: bool,
    pub args: Vec<Type>,
    pub returns: Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Trait {
    pub functions: Vec<TraitFunc>,
    pub declared_at: Option<Span>,
}

pub const fn any() -> TypeVariant {
    TypeVariant::Primitive(PrimitiveType::Any)
}

pub const fn integer() -> TypeVariant {
    TypeVariant::Primitive(PrimitiveType::Integer)
}

pub const fn float() -> TypeVariant {
    TypeVariant::Primitive(PrimitiveType::Float)
}

pub const fn string() -> TypeVariant {
    TypeVariant::Primitive(PrimitiveType::String)
}

pub const fn boolean() -> TypeVariant {
    TypeVariant::Primitive(PrimitiveType::Boolean)
}

pub const fn void() -> TypeVariant {
    TypeVariant::Primitive(PrimitiveType::Void)
}

pub fn array_of<T: Into<Type>>(element: T, size: Option<usize>) -> TypeVariant {
    TypeVariant::complex(ComplexType::Array(ArrayType { element: element.into(), size }))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComplexType {
    Function(FunctionType),
    Component(ComponentType),
    Struct(StructType),
    Array(ArrayType),
}

impl fmt::Display for ComplexType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function(value) => value.fmt(f),
            Self::Component(value) => value.fmt(f),
            Self::Struct(value) => value.fmt(f),
            Self::Array(value) => value.fmt(f),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeVariant {
    Primitive(PrimitiveType),
    Complex(Arc<ComplexType>),
}

impl TypeVariant {
    pub fn kind(&self) -> TypeKind {
        match self {
            Self::Primitive(PrimitiveType::Any) => TypeKind::Any,
            Self::Primitive(PrimitiveType::Integer) => TypeKind::Integer,
            Self::Primitive(PrimitiveType::Float) => TypeKind::Float,
            Self::Primitive(PrimitiveType::Boolean) => TypeKind::Boolean,
            Self::Primitive(PrimitiveType::String) => TypeKind::String,
            Self::Primitive(PrimitiveType::Component) => TypeKind::Component,
            Self::Primitive(PrimitiveType::Void) => TypeKind::Void,
            Self::Complex(ty) => match &**ty {
                ComplexType::Function(_) => TypeKind::Function,
                ComplexType::Component(_) => TypeKind::Component,
                ComplexType::Struct(_) => TypeKind::Struct,
                ComplexType::Array(array) => TypeKind::Array(Box::new(array.element.variant.kind()), array.size),
            },
        }
    }
}

impl fmt::Display for TypeVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(primitive) => primitive.fmt(f),
            Self::Complex(complex) => complex.fmt(f),
        }
    }
}

impl TypeVariant {
    pub fn complex(ty: ComplexType) -> Self {
        Self::Complex(Arc::new(ty))
    }

    pub fn same_as(&self, expected: &Self) -> bool {
        if matches!(expected, Self::Primitive(PrimitiveType::Any)) {
            true
        } else {
            match (self, expected) {
                (Self::Primitive(a), Self::Primitive(b)) => a == b,
                (Self::Complex(a), Self::Complex(b)) => match (a.as_ref(), b.as_ref()) {
                    (ComplexType::Array(got), ComplexType::Array(expected)) => {
                        got.element.variant.same_as(&expected.element.variant) && expected.size.is_none_or(|size| got.size.is_some_and(|got| got == size))
                    }
                    (a, b) => a == b,
                },
                _ => false,
            }
        }
    }

    pub fn as_struct(&self) -> Option<&StructType> {
        self.as_complex()
            .and_then(|complex| if let ComplexType::Struct(ty) = complex { Some(ty) } else { None })
    }

    pub fn as_component(&self) -> Option<&ComponentType> {
        self.as_complex()
            .and_then(|complex| if let ComplexType::Component(ty) = complex { Some(ty) } else { None })
    }

    pub fn as_array(&self) -> Option<&ArrayType> {
        self.as_complex()
            .and_then(|complex| if let ComplexType::Array(ty) = complex { Some(ty) } else { None })
    }

    pub fn as_function(&self) -> Option<&FunctionType> {
        self.as_complex()
            .and_then(|complex| if let ComplexType::Function(ty) = complex { Some(ty) } else { None })
    }

    pub fn is_struct(&self) -> bool {
        self.as_complex().is_some_and(|complex| matches!(complex, ComplexType::Struct(_)))
    }

    pub fn is_component(&self) -> bool {
        self.as_complex().is_some_and(|complex| matches!(complex, ComplexType::Component(_)))
    }

    pub fn is_array(&self) -> bool {
        self.as_complex().is_some_and(|complex| matches!(complex, ComplexType::Array(_)))
    }

    pub fn is_function(&self) -> bool {
        self.as_complex().is_some_and(|complex| matches!(complex, ComplexType::Function(_)))
    }

    pub fn as_complex(&self) -> Option<&ComplexType> {
        if let Self::Complex(complex_type) = self {
            Some(&**complex_type)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Type {
    pub variant: TypeVariant,
    pub declared_at: Option<Span>,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(declared_at) = self.declared_at {
            write!(f, "{} declared at {}:{}", self.variant, declared_at.line + 1, declared_at.column + 1)
        } else {
            self.variant.fmt(f)
        }
    }
}

impl Type {
    pub fn kind(&self) -> MaybePositioned<TypeKind> {
        MaybePositioned {
            value: self.variant.kind(),
            span: self.declared_at.map(|span| (SpanType::Definition, span)),
        }
    }
}

impl From<TypeVariant> for Type {
    fn from(value: TypeVariant) -> Self {
        Self {
            variant: value,
            declared_at: None,
        }
    }
}

pub fn function<T: IntoIterator<Item = TypeVariant>, R: Into<Type>>(have_self: bool, args: T, returns: R) -> TypeVariant {
    TypeVariant::complex(ComplexType::Function(FunctionType {
        have_self,
        args: args.into_iter().map(Into::into).collect(),
        returns: Box::new(returns.into()),
    }))
}

pub fn structure<K: Into<String>, V: Into<Type>, T: IntoIterator<Item = (K, V)>>(properties: T) -> TypeVariant {
    TypeVariant::complex(ComplexType::Struct(StructType {
        properties: properties.into_iter().map(|(key, value)| (key.into(), value.into())).collect(),
    }))
}

pub fn component<K: Into<String>, V: Into<Type>, T: IntoIterator<Item = (K, bool, V)>>(properties: T, children: ComponentChildren) -> TypeVariant {
    TypeVariant::complex(ComplexType::Component(ComponentType {
        properties: properties
            .into_iter()
            .map(|(key, nullable, value)| (key.into(), nullable, value.into()))
            .collect(),
        children,
    }))
}
