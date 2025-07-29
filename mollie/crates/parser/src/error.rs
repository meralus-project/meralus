use mollie_lexer::Token;
use mollie_shared::{Positioned, Span};

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError(pub String, pub Option<Span>);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    pub fn new<T: Into<String>>(value: T, span: Option<Span>) -> Self {
        Self(value.into(), span)
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub const fn location(&self) -> Option<Span> {
        self.1
    }

    pub fn unexpected_token(token: Option<&Positioned<Token>>) -> Self {
        token.map_or_else(
            || Self("Unexpected nothing".to_string(), None),
            |token| Self(format!("Unexpected {}", token.value), Some(token.span)),
        )
    }

    pub fn expected_token(expected: &Token, found: Option<&Positioned<Token>>) -> Self {
        found.map_or_else(
            || Self(format!("Expected {expected}, found nothing"), None),
            |found| Self(format!("Expected {expected}, found {}", found.value), Some(found.span)),
        )
    }

    pub fn expected_tokens(expected: &[Token], found: Option<&Positioned<Token>>) -> Self {
        let expected = match expected.len() {
            0 => "nothing".into(),
            1 => expected[0].to_string(),
            value => {
                format!(
                    "{}{} or {}",
                    expected[0],
                    expected[1..value - 1].iter().map(ToString::to_string).collect::<Vec<_>>().join(", "),
                    expected[value - 1]
                )
            }
        };

        found.map_or_else(
            || Self(format!("Expected {expected}, found nothing"), None),
            |found| Self(format!("Expected {expected}, found {}", found.value), Some(found.span)),
        )
    }

    #[must_use]
    pub fn too_much_tokens(tokens: &[Positioned<Token>]) -> Self {
        Self(format!("found more than zero ({}) tokens after parsing: {tokens:#?}", tokens.len()), None)
    }
}
