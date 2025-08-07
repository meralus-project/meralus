use std::fmt::{self, Write};

use mollie_shared::pretty_fmt::{PrettyFmt, indent_down, indent_up};
use serde::{Deserialize, Serialize};

use crate::Type;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructType {
    pub generics: Vec<String>,
    pub properties: Vec<(String, Type)>,
}

impl fmt::Display for StructType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("struct {\n")?;

        indent_down();

        f.write_array_like(self.properties.iter().map(|(key, value)| format!("{key}: {value}")), true)?;

        indent_up();

        f.write_char('\n')?;
        f.write_indent()?;
        f.write_char('}')
    }
}
