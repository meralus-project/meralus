mod maybe_positioned;
mod positioned;
pub mod pretty_fmt;
mod span;

use std::fmt;

pub use self::{
    maybe_positioned::{MaybePositioned, SpanType},
    positioned::Positioned,
    span::Span,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operator {
    Assign,      // =
    Add,         // +
    Sub,         // -
    Mul,         // *
    Div,         // /
    Equal,       // ==
    NotEqual,    // !=
    LessThan,    // <
    GreaterThan, // >
    And,         // &&
    Or,          // ||
    Is,          // is
}

impl Operator {
    pub const fn need_same_types(&self) -> bool {
        !matches!(self, Self::Assign | Self::Is)
    }

    pub const fn produce_same_type(&self) -> bool {
        matches!(self, Self::Add | Self::Sub | Self::Mul | Self::Div)
    }
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Assign => "=",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::LessThan => "<",
            Self::GreaterThan => ">",
            Self::And => "&&",
            Self::Or => "||",
            Self::Is => "is",
        })
    }
}
