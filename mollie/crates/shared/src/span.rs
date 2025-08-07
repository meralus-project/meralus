use serde::{Deserialize, Serialize};

use crate::Positioned;

#[derive(Debug, Default, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self { start, end, line, column }
    }

    #[must_use]
    pub const fn between(&self, to: Self) -> Self {
        Self {
            start: self.start,
            end: to.end,
            line: self.line,
            column: self.column,
        }
    }

    pub const fn wrap<T>(self, value: T) -> Positioned<T> {
        Positioned { value, span: self }
    }
}
