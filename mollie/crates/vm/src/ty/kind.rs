use std::fmt;

#[derive(Debug, Clone)]
pub enum TypeKind {
    Void,
    Any,
    Integer,
    Float,
    Boolean,
    String,
    Trait,
    Struct,
    Enum,
    Function,
    Component,
    Array(Box<Self>, Option<usize>),
    OneOf(Vec<Self>),
    Null,
    Generic,
}

impl fmt::Display for TypeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Void => f.write_str("void"),
            Self::Any => f.write_str("<any>"),
            Self::Generic => f.write_str("<generic>"),
            Self::Integer => f.write_str("integer"),
            Self::Float => f.write_str("float"),
            Self::Boolean => f.write_str("boolean"),
            Self::Null => f.write_str("null"),
            Self::String => f.write_str("string"),
            Self::Trait => f.write_str("trait"),
            Self::Struct => f.write_str("structure"),
            Self::Enum => f.write_str("enumeration"),
            Self::Function => f.write_str("function"),
            Self::Component => f.write_str("component"),
            Self::Array(element, size) => write!(f, "{element}[{}]", size.as_ref().map_or_else(String::new, ToString::to_string)),
            Self::OneOf(kinds) => {
                let mut first = true;

                for kind in kinds {
                    if first {
                        first = false;

                        write!(f, "{kind}")?;
                    } else {
                        write!(f, " | {kind}")?;
                    }
                }

                Ok(())
            }
        }
    }
}
