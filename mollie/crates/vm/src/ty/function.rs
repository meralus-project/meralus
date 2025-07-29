use std::fmt;

use mollie_shared::pretty_fmt::PrettyFmt;

use crate::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionType {
    pub have_self: bool,
    pub args: Vec<Type>,
    pub returns: Box<Type>,
}

impl fmt::Display for FunctionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fn(")?;
        f.write_array_like(&self.args, false)?;
        
        write!(f, ") -> {}", self.returns)
    }
}
