mod token;

use std::{
    iter::{self, Peekable},
    ops::Neg,
    str::Chars,
};

use mollie_shared::{Positioned, Span};

pub use crate::token::Token;

pub struct Lexer;

impl Lexer {
    fn lex_reserved(ident: String) -> Token {
        match ident.as_str() {
            "true" => Token::Boolean(true),
            "false" => Token::Boolean(false),
            "self" => Token::This,
            "declare" => Token::Declare,
            "inherits" => Token::Inherits,
            "as" => Token::As,
            "from" => Token::From,
            "struct" => Token::Struct,
            "enum" => Token::Enum,
            // "match" => Token::Match,
            "fn" => Token::Fn,
            "trait" => Token::Trait,
            "impl" => Token::Impl,
            "const" => Token::Const,
            "let" => Token::Let,
            "while" => Token::While,
            "for" => Token::For,
            // "infix" => Token::Infix,
            "in" => Token::In,
            "loop" => Token::Loop,
            "if" => Token::If,
            "else" => Token::Else,
            "null" => Token::Null,
            "is" => Token::Is,
            _ => Token::Ident(ident),
        }
    }

    fn lex_other(chars: &mut Peekable<Chars>, span: &mut Span, character: char) -> Token {
        match character {
            '[' => Token::BracketOpen,
            ']' => Token::BracketClose,
            '{' => Token::BraceOpen,
            '}' => Token::BraceClose,
            '(' => Token::ParenOpen,
            ')' => Token::ParenClose,
            ':' => {
                if chars.next_if_eq(&':').is_some() {
                    span.end += 1;
                    span.column += 1;

                    Token::PathSep
                } else {
                    Token::Colon
                }
            }
            ';' => Token::Semi,
            '+' => Token::Plus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '=' => {
                if chars.next_if_eq(&'=').is_some() {
                    span.end += 1;
                    span.column += 1;

                    Token::EqEq
                } else {
                    Token::Eq
                }
            }
            '&' => {
                if chars.next_if_eq(&'&').is_some() {
                    span.end += 1;
                    span.column += 1;

                    Token::AndAnd
                } else {
                    Token::And
                }
            }
            '|' => {
                if chars.next_if_eq(&'|').is_some() {
                    span.end += 1;
                    span.column += 1;

                    Token::OrOr
                } else {
                    Token::Or
                }
            }
            '%' => Token::Percent,
            '.' => {
                if chars.next_if_eq(&'.').is_some() {
                    span.end += 1;
                    span.column += 1;

                    Token::DotDot
                } else {
                    Token::Dot
                }
            }
            ',' => Token::Comma,
            '!' => {
                if chars.next_if_eq(&'=').is_some() {
                    span.end += 1;
                    span.column += 1;

                    Token::NotEq
                } else {
                    Token::Not
                }
            }
            // '#' => Token::Pound,
            '?' => Token::Question,
            '>' => Token::Greater,
            '<' => Token::Less,
            character => Token::Unknown(character),
        }
    }

    fn parse_string(chars: &mut Peekable<Chars>) -> (Token, usize, usize) {
        let mut size = 0;
        let mut utf8size = 0;
        let mut data = String::new();
        // let mut datas = Vec::new();
        // let mut formatted = Vec::new();

        while let Some(character) = chars.next_if(|character| character != &'"') {
            size += 1;
            utf8size += character.len_utf8();

            if character == '\\'
                && let Some(character) = chars.next()
            {
                size += 1;
                utf8size += character.len_utf8();

                data.push(character);
            }

            //         if character == '{' {
            //             datas.push(mem::take(&mut data));

            //             formatted.push(
            //                 iter::from_fn(|| {
            //                     chars.next_if(|&s| s != '{' && s != '}').inspect(|value|
            // {                         size += 1;
            //                         utf8size += value.len_utf8();
            //                     })
            //                 })
            //                 .collect::<String>(),
            //             );

            //             size += 1;
            //             utf8size += 1;

            //             chars.next_if_eq(&'}');
            //         } else {
            data.push(character);
            // }
        }

        chars.next_if_eq(&'"');

        // if datas.is_empty() && formatted.is_empty() {
        (Token::String(data), size, utf8size)
        //     } else {
        //         let mut parts = datas
        //             .into_iter()
        //             .map(StringPart::String)
        //             .zip(formatted.into_iter().map(|value|
        // StringPart::Formatted(Self::parse(value))))
        // .fold(Vec::new(), |mut parts, tuple| {
        // parts.extend(<[StringPart; 2]>::from(tuple));

        //                 parts
        //             });

        //         parts.push(StringPart::String(data));

        //         (Token::FormattedString(parts), size, utf8size)
        //     }
    }

    fn lex_number(chars: &mut Peekable<Chars>, tokens: &mut Vec<Positioned<Token>>, span: &mut Span, character: char, neg: bool) {
        let mut value = iter::once(character)
            .chain(iter::from_fn(|| chars.by_ref().next_if(char::is_ascii_digit)))
            .collect::<String>();

        if value == "0" && chars.next_if_eq(&'x').is_some() {
            let hex = iter::from_fn(|| chars.by_ref().next_if(char::is_ascii_hexdigit)).collect::<String>();

            span.start = span.end;
            span.end += hex.len() + 2;

            tokens.push(span.wrap(Token::Integer(i64::from_str_radix(&hex, 16).unwrap(), None, true)));
        } else {
            if chars.next_if_eq(&'.').is_some() && chars.peek().is_some_and(char::is_ascii_digit) {
                value.push('.');
                value.push_str(&iter::from_fn(|| chars.by_ref().next_if(char::is_ascii_digit)).collect::<String>());
            }

            let postfix = if chars.peek().is_some_and(|c| c.is_ascii_alphabetic() || c == &'%') {
                if chars.next_if_eq(&'%').is_some() {
                    Some("%".to_string())
                } else {
                    Some(iter::from_fn(|| chars.by_ref().next_if(char::is_ascii_alphabetic)).collect::<String>())
                }
            } else {
                None
            };

            span.start = span.end;
            span.end += value.len() + postfix.as_ref().map(String::len).unwrap_or_default();

            if value.contains('.') {
                let mut value: f32 = value.parse().unwrap();

                if neg {
                    value = value.neg();
                }

                tokens.push(span.wrap(Token::Float(value, postfix)));
            } else {
                let mut value: i64 = value.parse().unwrap();

                if neg {
                    value = value.neg();
                }

                tokens.push(span.wrap(Token::Integer(value, postfix, false)));
            }

            span.column += value.len();
        }
    }

    /// # Panics
    ///
    /// Can panic if number failed to parse
    pub fn lex<T: AsRef<str>>(data: T) -> Vec<Positioned<Token>> {
        let mut tokens = vec![];
        let mut chars = data.as_ref().chars().peekable();
        let mut span = Span::new(0, 0, 0, 0);

        while let Some(character) = chars.next() {
            match character {
                'A'..='Z' | 'a'..='z' => {
                    let ident = iter::once(character)
                        .chain(iter::from_fn(|| {
                            chars.by_ref().next_if(|s| s.is_ascii_alphanumeric() || s == &'-' || s == &'_')
                        }))
                        .collect::<String>();

                    span.start = span.end;
                    span.end += ident.len();
                    span.column += ident.len();

                    tokens.push(span.wrap(Self::lex_reserved(ident)));
                }
                '0'..='9' => {
                    Self::lex_number(&mut chars, &mut tokens, &mut span, character, false);
                }
                '-' => {
                    if let Some(character) = chars.next_if(char::is_ascii_digit) {
                        Self::lex_number(&mut chars, &mut tokens, &mut span, character, true);
                    } else if chars.next_if_eq(&'>').is_some() {
                        span.start = span.end;
                        span.end += 2;

                        tokens.push(span.wrap(Token::Arrow));

                        span.column += 2;
                    } else {
                        span.start = span.end;
                        span.end += 1;

                        tokens.push(span.wrap(Token::Minus));

                        span.column += 1;
                    }
                }
                '"' => {
                    span.start = span.end;

                    let (value, size, utf8size) = Self::parse_string(&mut chars);

                    span.start = span.end;
                    span.end += utf8size + 2;
                    span.column += size + 2;

                    tokens.push(span.wrap(value));
                }
                character => {
                    span.start = span.end;
                    span.end += 1;

                    if character.is_ascii_whitespace() {
                        if character == '\n' {
                            span.line += 1;
                            span.column = 0;
                        } else {
                            span.column += 1;
                        }

                        continue;
                    }

                    let token = Self::lex_other(&mut chars, &mut span, character);

                    tokens.push(span.wrap(token));

                    span.column += 1;
                }
            }
        }

        tokens.push(span.wrap(Token::EOF));

        tokens
    }
}
