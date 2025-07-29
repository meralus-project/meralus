use std::fmt;

use crate::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanType {
    Simple,
    Definition,
}

impl fmt::Display for SpanType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Simple => f.write_str("at"),
            Self::Definition => f.write_str("defined at"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaybePositioned<T> {
    pub value: T,
    pub span: Option<(SpanType, Span)>,
}

impl<T> MaybePositioned<T> {
    pub const fn new(value: T, span_type: SpanType, span: Span) -> Self {
        Self {
            value,
            span: Some((span_type, span)),
        }
    }
}

impl<T> From<T> for MaybePositioned<T> {
    fn from(value: T) -> Self {
        Self { value, span: None }
    }
}

impl<T: fmt::Display> fmt::Display for MaybePositioned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.span {
            Some((span_type, span)) => write!(f, "{} {span_type} {}:{}", self.value, span.line + 1, span.column + 1),
            None => self.value.fmt(f),
        }
    }
}
