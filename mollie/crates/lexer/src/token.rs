use derive_more::derive::{Display, IsVariant, Unwrap};

#[derive(Debug, Display, IsVariant, Unwrap, PartialEq, Clone, PartialOrd)]
pub enum Token {
    #[is_variant]
    #[unwrap]
    Ident(String),
    // #[display("\"{}\"", _0.iter().map(ToString::to_string).collect::<String>())]
    // #[is_variant]
    // FormattedString(Vec<StringPart>),
    // #[display("\"{_0}\"")]
    #[display("{_0:?}")]
    #[is_variant]
    #[unwrap]
    String(String),
    #[display("{_0}")]
    #[is_variant]
    #[unwrap]
    Float(f32, Option<String>),
    #[display("{}", if *_2 { format!("0x{_0:06X}{}", _1.as_deref().unwrap_or_default()) } else { format!("{_0}{}", _1.as_deref().unwrap_or_default()) })]
    #[is_variant]
    #[unwrap]
    Integer(i64, Option<String>, bool),
    #[display("{_0}")]
    #[is_variant]
    #[unwrap]
    Boolean(bool),
    #[display("self")]
    This,
    #[display("declare")]
    Declare,
    #[display("inherits")]
    Inherits,
    #[display("as")]
    As,
    #[display("is")]
    Is,
    #[display("from")]
    From,
    #[display("..")]
    DotDot,
    #[display("enum")]
    Enum,
    #[display("struct")]
    Struct,
    #[display("const")]
    Const,
    #[display("let")]
    Let,
    #[display("while")]
    While,
    #[display("for")]
    For,
    #[display("in")]
    In,
    #[display("if")]
    If,
    #[display("else")]
    Else,
    #[display("loop")]
    Loop,
    #[display("null")]
    Null,
    // #[display("infix")]
    // Infix,
    #[display("fn")]
    Fn,
    // #[display("match")]
    // Match,
    // #[display("is")]
    // Is,
    #[display("[")]
    BracketOpen,
    #[display("]")]
    BracketClose,
    #[display("{{")]
    BraceOpen,
    #[display("}}")]
    BraceClose,
    #[display("(")]
    ParenOpen,
    #[display(")")]
    ParenClose,
    #[display("::")]
    PathSep,
    #[display(":")]
    Colon,
    #[display(";")]
    Semi,
    #[display("-")]
    Minus,
    #[display("+")]
    Plus,
    #[display("/")]
    Slash,
    #[display("*")]
    Star,
    // #[display("=>")]
    // FatArrow,
    #[display("->")]
    Arrow,
    #[display("=")]
    Eq,
    #[display("==")]
    EqEq,
    #[display("!=")]
    NotEq,
    #[display("!")]
    Not,
    #[display("|")]
    Or,
    #[display("||")]
    OrOr,
    #[display("&")]
    And,
    #[display("&&")]
    AndAnd,
    // #[display("#")]
    // Pound,
    #[display("?")]
    Question,
    #[display("%")]
    Percent,
    #[display(",")]
    Comma,
    #[display("<")]
    Less,
    #[display(">")]
    Greater,
    #[display(".")]
    Dot,
    // #[display("new")]
    // New,
    #[display("impl")]
    Impl,
    #[display("trait")]
    Trait,
    #[display("{_0}")]
    Unknown(char),
    #[display("<EOF>")]
    #[is_variant]
    EOF,
}

impl Token {
    pub fn ident<T: Into<String>>(value: T) -> Self {
        Self::Ident(value.into())
    }

    #[must_use]
    pub fn is_ident_and(&self, func: impl FnOnce(&String) -> bool) -> bool {
        if let Self::Ident(value) = self { func(value) } else { false }
    }

    // #[must_use]
    // pub const fn is_number(&self) -> bool {
    //     self.is_float() || self.is_integer()
    // }

    // #[must_use]
    // pub const fn is_usize(&self) -> bool {
    //     if let Self::Integer(value, _) = self {
    //         *value >= 0
    //     } else {
    //         false
    //     }
    // }

    // #[must_use]
    // pub fn into_usize(self) -> usize {
    //     if let Self::Integer(value, _) = self {
    //         let Ok(value) = value.try_into() else {
    //             unreachable!()
    //         };

    //         value
    //     } else {
    //         unreachable!()
    //     }
    // }
}
