use std::fmt;

use crate::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayType {
    pub element: Type,
    pub size: Option<usize>,
}

impl fmt::Display for ArrayType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]", self.element, self.size.as_ref().map_or_else(String::new, ToString::to_string))
    }
}
