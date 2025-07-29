use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrimitiveType {
    Any,
    Integer,
    Float,
    Boolean,
    String,
    Component,
    Void,
}

impl fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => f.write_str("<any>"),
            Self::Integer => f.write_str("integer"),
            Self::Float => f.write_str("float"),
            Self::Boolean => f.write_str("boolean"),
            Self::String => f.write_str("string"),
            Self::Component => f.write_str("component"),
            Self::Void => f.write_str("void"),
        }
    }
}

