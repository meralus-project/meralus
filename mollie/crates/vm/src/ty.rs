mod array;
mod component;
mod enumeration;
mod function;
mod kind;
mod primitive;
mod structure;

use std::{
    fmt::{self, Write},
    sync::Arc,
};

use mollie_shared::{MaybePositioned, Span, SpanType, pretty_fmt::PrettyFmt};
use serde::{Deserialize, Serialize};

pub use self::{
    array::ArrayType,
    component::{ComponentChildren, ComponentType},
    enumeration::{EnumType, EnumVariant},
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
    pub generics: Vec<String>,
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

pub const fn null() -> TypeVariant {
    TypeVariant::Primitive(PrimitiveType::Null)
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "kebab-case")]
pub enum ComplexType {
    Function(FunctionType),
    Component(ComponentType),
    Struct(StructType),
    EnumType(EnumType),
    Array(ArrayType),
    TraitInstance(Type, usize),
    OneOf(Vec<Type>),
}

impl fmt::Display for ComplexType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function(value) => value.fmt(f),
            Self::Component(value) => value.fmt(f),
            Self::Struct(value) => value.fmt(f),
            Self::EnumType(value) => value.fmt(f),
            Self::Array(value) => value.fmt(f),
            Self::TraitInstance(value, _) => value.fmt(f),
            Self::OneOf(v) => {
                let mut first = true;

                for v in v {
                    if first {
                        v.fmt(f)?;

                        first = false;
                    } else {
                        write!(f, " | {v}")?;
                    }
                }

                Ok(())
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "kebab-case")]
pub enum TypeVariant {
    Primitive(PrimitiveType),
    Complex(Arc<ComplexType>),
    Generic(usize),
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
                ComplexType::TraitInstance(ty, _) => ty.variant.kind(),
                ComplexType::Struct(_) => TypeKind::Struct,
                ComplexType::EnumType(_) => TypeKind::Enum,
                ComplexType::Array(array) => TypeKind::Array(Box::new(array.element.variant.kind()), array.size),
                ComplexType::OneOf(types) => TypeKind::OneOf(types.iter().map(|t| t.kind().value).collect()),
            },
            Self::Generic(_) => TypeKind::Generic,
            Self::Primitive(PrimitiveType::Null) => TypeKind::Null,
        }
    }
}

impl fmt::Display for TypeVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(primitive) => primitive.fmt(f),
            Self::Complex(complex) => complex.fmt(f),
            Self::Generic(_) => f.write_str("<generic>"),
        }
    }
}

impl TypeVariant {
    pub fn complex(ty: ComplexType) -> Self {
        Self::Complex(Arc::new(ty))
    }

    pub fn same_as(&self, expected: &Self, applied_generics: &[Type]) -> bool {
        if let (Self::Generic(a), Self::Generic(b)) = (self, expected) {
            return a == b;
        }

        let me = if let Self::Generic(generic) = self {
            &applied_generics[*generic].variant
        } else {
            self
        };

        let expected = if let Self::Generic(generic) = expected {
            &applied_generics[*generic].variant
        } else {
            expected
        };

        if matches!(expected, Self::Primitive(PrimitiveType::Any)) {
            true
        } else if matches!(me, Self::Primitive(PrimitiveType::Null)) {
            true
        } else {
            match (me, expected) {
                (Self::Primitive(a), Self::Primitive(b)) => a == b,
                (Self::Generic(a), Self::Generic(b)) => a == b,
                (me, Self::Complex(b)) => {
                    if let ComplexType::OneOf(types) = b.as_ref() {
                        if let Some(ComplexType::OneOf(a_types)) = me.as_complex()
                            && a_types == types
                        {
                            true
                        } else {
                            for ty in types {
                                if me.same_as(&ty.variant, applied_generics) {
                                    return true;
                                }
                            }

                            false
                        }
                    } else if let Self::Complex(a) = me {
                        match (a.as_ref(), b.as_ref()) {
                            (ComplexType::Array(got), ComplexType::Array(expected)) => {
                                got.element.variant.same_as(&expected.element.variant, applied_generics)
                                    && expected.size.is_none_or(|size| got.size.is_some_and(|got| got == size))
                            }
                            (a, b) => a == b,
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            }
        }
    }

    pub fn as_enum(&self) -> Option<&EnumType> {
        self.as_complex()
            .and_then(|complex| if let ComplexType::EnumType(ty) = complex { Some(ty) } else { None })
    }

    pub fn as_struct(&self) -> Option<&StructType> {
        self.as_complex()
            .and_then(|complex| if let ComplexType::Struct(ty) = complex { Some(ty) } else { None })
    }

    pub fn as_component(&self) -> Option<&ComponentType> {
        self.as_complex()
            .and_then(|complex| if let ComplexType::Component(ty) = complex { Some(ty) } else { None })
    }

    pub fn as_trait_instance(&self) -> Option<(&Type, usize)> {
        self.as_complex().and_then(|complex| {
            if let ComplexType::TraitInstance(ty, index) = complex {
                Some((ty, *index))
            } else {
                None
            }
        })
    }

    pub fn as_array(&self) -> Option<&ArrayType> {
        self.as_complex()
            .and_then(|complex| if let ComplexType::Array(ty) = complex { Some(ty) } else { None })
    }

    pub fn as_function(&self) -> Option<&FunctionType> {
        self.as_complex()
            .and_then(|complex| if let ComplexType::Function(ty) = complex { Some(ty) } else { None })
    }

    pub fn is_enum(&self) -> bool {
        self.as_complex().is_some_and(|complex| matches!(complex, ComplexType::EnumType(_)))
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct Type {
    pub variant: TypeVariant,
    pub applied_generics: Vec<Type>,
    pub declared_at: Option<Span>,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.variant.fmt(f)?;

        if !self.applied_generics.is_empty() {
            f.write_char('<')?;
            f.write_array_like(&self.applied_generics, false)?;
            f.write_char('>')?;
        }

        if let Some(declared_at) = self.declared_at {
            write!(f, " declared at {}:{}", declared_at.line + 1, declared_at.column + 1)?;
        }

        Ok(())
    }
}

impl Type {
    fn set_first_generic(&mut self, value: Type) {
        if self.applied_generics.is_empty() {
            self.applied_generics.push(value);
        } else {
            self.applied_generics[0] = value;
        }
    }

    pub fn inherit_from(&mut self, other: &TypeVariant) {
        if let Some(array) = other.as_array() {
            match &self.variant {
                TypeVariant::Complex(complex_type) => match &**complex_type {
                    ComplexType::Function(function_type) => {
                        if function_type.args.iter().any(|arg| matches!(arg.variant, TypeVariant::Generic(0))) {
                            self.set_first_generic(array.element.clone());
                        } else if matches!(function_type.returns.variant, TypeVariant::Generic(0)) {
                            self.set_first_generic(array.element.clone());
                        }
                    }
                    ComplexType::Struct(struct_type) => {
                        if !struct_type.generics.is_empty() {
                            self.set_first_generic(array.element.clone());
                        }
                    }
                    ComplexType::EnumType(_) => {}
                    ComplexType::Array(array_type) => {
                        if matches!(array_type.element.variant, TypeVariant::Generic(0)) {
                            self.set_first_generic(array.element.clone());
                        }
                    }
                    ComplexType::OneOf(items) => {
                        if items.iter().any(|item| matches!(item.variant, TypeVariant::Generic(0))) {
                            self.set_first_generic(array.element.clone());
                        }
                    }
                    _ => {
                        // if !component_type.generics.is_empty() {
                        //     self.applied_generics[0] = array.element.clone();
                        // }
                    }
                },
                TypeVariant::Generic(0) => self.set_first_generic(array.element.clone()),
                _ => {}
            }
        }
    }

    #[must_use]
    pub fn resolve_type(mut self, applied_generics: &[Self]) -> Self {
        if let TypeVariant::Generic(generic) = self.variant {
            applied_generics[generic].clone()
        } else if let TypeVariant::Complex(variant) = &self.variant {
            if let ComplexType::OneOf(types) = &**variant {
                let mut result_types = Vec::with_capacity(types.len());

                for ty in types {
                    result_types.push(ty.clone().resolve_type(applied_generics));
                }

                Self {
                    variant: TypeVariant::complex(ComplexType::OneOf(result_types)),
                    applied_generics: self.applied_generics,
                    declared_at: self.declared_at,
                }
            } else if self.variant.is_enum() || self.variant.is_struct() {
                self.applied_generics = applied_generics.to_vec();

                self
            } else {
                self
            }
        } else {
            self
        }
    }

    pub fn resolved_kind(&self, applied_generics: &[Self]) -> MaybePositioned<TypeKind> {
        if let TypeVariant::Generic(generic) = self.variant {
            applied_generics.get(generic).map_or_else(|| self.kind(), Self::kind)
        } else {
            self.kind()
        }
    }

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
            applied_generics: Vec::new(),
            variant: value,
            declared_at: None,
        }
    }
}

pub fn function<T: IntoIterator<Item = TypeVariant>, R: Into<Type>>(have_self: bool, args: T, returns: R) -> TypeVariant {
    TypeVariant::complex(ComplexType::Function(FunctionType {
        is_native: true,
        have_self,
        args: args.into_iter().map(Into::into).collect(),
        returns: Box::new(returns.into()),
    }))
}

pub fn structure<K: Into<String>, V: Into<Type>, T: IntoIterator<Item = (K, V)>>(properties: T) -> TypeVariant {
    TypeVariant::complex(ComplexType::Struct(StructType {
        generics: Vec::new(),
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
