use std::fmt;

use mollie_parser::ParseError;
use mollie_shared::MaybePositioned;
use mollie_vm::{ComponentChildren, Type, TypeKind};

pub type CompileResult<T = bool> = Result<T, CompileError>;
pub type TypeResult<T = Type> = Result<T, TypeError>;

#[derive(Debug)]
pub enum CompileError {
    Type(TypeError),
    Parse(ParseError),
    VariableNotFound {
        name: String,
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Type(value) => value.fmt(f),
            Self::Parse(value) => value.fmt(f),
            Self::VariableNotFound { name } => write!(f, "there's no variable called {name}"),
        }
    }
}

impl std::error::Error for CompileError {}

impl From<TypeError> for CompileError {
    fn from(value: TypeError) -> Self {
        Self::Type(value)
    }
}

#[non_exhaustive]
#[derive(Debug)]
pub enum TypeError {
    Unexpected {
        got: Box<MaybePositioned<TypeKind>>,
        expected: Box<MaybePositioned<TypeKind>>,
    },
    NotFound {
        ty: Option<Box<TypeKind>>,
        name: String,
    },
    FunctionDefinitionInvalid {
        name: String,
    },
    UnknownTraitFunction {
        trait_name: String,
        name: String,
    },
    TraitFunctionNotFound {
        trait_name: String,
        name: String,
    },
    FunctionNotFound {
        ty: Box<TypeKind>,
        ty_name: Option<String>,
        function: String,
    },
    PropertyNotFound {
        ty: Box<TypeKind>,
        ty_name: Option<String>,
        property: String,
    },
    InvalidArgumentType {
        got: Box<MaybePositioned<TypeKind>>,
        expected: Box<MaybePositioned<TypeKind>>,
    },
    InvalidArguments {
        got: usize,
        expected: usize,
    },
    InvalidChildren {
        got: ComponentChildren,
        expected: ComponentChildren,
        ty_name: String,
    },
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unexpected { got, expected } => write!(f, "expected `{expected}`, received `{got}`"),
            Self::NotFound { ty, name } => write!(
                f,
                "there's no {} called `{name}`",
                ty.as_ref().map_or_else(|| String::from("type"), |ty| format!("`{ty}` type"))
            ),
            Self::UnknownTraitFunction { trait_name, name } => write!(f, "there's no `{name}` in `{trait_name}` trait"),
            Self::InvalidArgumentType { got, expected } => write!(f, "expected argument type to be `{expected}`, found `{got}`"),
            Self::TraitFunctionNotFound { trait_name, name } => write!(f, "`{name}` wasn't found in implementation of  `{trait_name}` trait"),
            Self::FunctionDefinitionInvalid { name } => write!(f, "found invalid definition of `{name}`"),
            Self::PropertyNotFound { ty, ty_name, property } => write!(
                f,
                "there's no `{property}` in `{ty}`{}",
                ty_name.as_ref().map_or_else(String::new, |ty_name| format!(" called `{ty_name}`"))
            ),
            Self::FunctionNotFound { ty, ty_name, function } => write!(
                f,
                "there's no function called `{function}` in `{ty}`{}",
                ty_name.as_ref().map_or_else(String::new, |ty_name| format!(" called `{ty_name}`"))
            ),            Self::InvalidArguments { got, expected } => write!(f, "expected `{expected}` arguments for function, received `{got}`"),
            Self::InvalidChildren { got, expected, ty_name } => match (expected, got) {
                (ComponentChildren::None, _) => write!(f, "component called `{ty_name}` cannot have children"),
                (ComponentChildren::Single, ComponentChildren::None) => write!(f, "component called `{ty_name}` must have one child"),
                (ComponentChildren::Single | ComponentChildren::MaybeSingle, ComponentChildren::Multiple(_)) => {
                    write!(f, "component called `{ty_name}` can have only one child")
                }
                (ComponentChildren::Multiple(_), ComponentChildren::None) => write!(f, "component called `{ty_name}` must have at least one child`"),
                _ => unreachable!(),
            },
        }
    }
}
