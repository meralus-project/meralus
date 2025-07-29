mod array;
mod block;
mod call;
mod ident;
mod index;
mod literal;
mod node;
mod type_index;

use mollie_lexer::Token;
use mollie_shared::Positioned;

pub use self::{
    array::ArrayExpr,
    block::{Block, parse_statements_until},
    call::FunctionCall,
    ident::Ident,
    index::IndexExpr,
    literal::{Literal, Number, SizeType},
    node::{Node, NodeProperty},
    type_index::TypeIndexExpr,
};
use crate::{Parse, ParseResult, Parser};

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Hash)]
pub enum Precedence {
    PLowest,
    POr,
    PAnd,
    PEquals,
    PLessGreater,
    PSum,
    PProduct,
    PCall,
    PIndex,
    PCheck,
}

impl Precedence {
    const fn from_ref(token: &Token) -> (Self, Option<()>) {
        match &token {
            // Token::AndAnd => (Self::PAnd, Some(Operator::And)),
            // Token::OrOr => (Self::POr, Some(Operator::Or)),
            // Token::Eq => (Self::PEquals, Some(Operator::Assign)),
            // Token::EqEq => (Self::PEquals, Some(Operator::Equal)),
            // Token::NotEq => (Self::PEquals, Some(Operator::NotEqual)),
            // Token::Less => (Self::PLessGreater, Some(Operator::LessThan)),
            // Token::Greater => (Self::PLessGreater, Some(Operator::GreaterThan)),
            // Token::Plus => (Self::PSum, Some(Operator::Add)),
            // Token::Minus => (Self::PSum, Some(Operator::Sub)),
            // Token::Star => (Self::PProduct, Some(Operator::Mul)),
            // Token::Slash => (Self::PProduct, Some(Operator::Div)),
            Token::ParenOpen => (Self::PCall, None),
            Token::BracketOpen | Token::Dot => (Self::PIndex, None),
            // Token::Is => (Self::PCheck, None),
            _ => (Self::PLowest, None),
        }
    }
}

fn go_parse_pratt_expr(parser: &mut Parser, precedence: Precedence, left: Positioned<Expression>) -> ParseResult<Positioned<Expression>> {
    if let Some(value) = parser.peek() {
        let (p, _) = Precedence::from_ref(&value.value);

        match p {
            // Precedence::PCheck if precedence < Precedence::PCheck => {
            //     let left = CheckExpression::parse(parser, left)?.map(Expression::Check);

            //     go_parse_pratt_expr(parser, precedence, left)
            // }
            Precedence::PCall if precedence < Precedence::PCall => {
                let left = FunctionCall::parse(parser, left)?.map(Expression::FunctionCall);

                go_parse_pratt_expr(parser, precedence, left)
            }
            Precedence::PIndex if precedence < Precedence::PIndex => {
                let left = IndexExpr::parse(parser, left)?.map(Expression::Index);

                go_parse_pratt_expr(parser, precedence, left)
            }
            // ref peek_precedence if precedence < *peek_precedence => {
            //     let left = BinaryExpression::parse(parser, left)?.map(Box::new).map(Expression::Binary);

            //     go_parse_pratt_expr(parser, precedence, left)
            // }
            _ => Ok(left),
        }
    } else {
        Ok(left)
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Expression {
    Literal(Literal),
    FunctionCall(FunctionCall),
    Node(Node),
    Index(IndexExpr),
    TypeIndex(TypeIndexExpr),
    Array(ArrayExpr),
    Ident(Ident),
    This,
}

impl Expression {
    fn parse_atom(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        if parser.check(&Token::ParenOpen) {
            let (_, mut parser) = parser.split(&Token::ParenOpen, &Token::ParenClose)?;

            Self::parse(&mut parser)
        } else {
            Literal::parse(parser)
                .map(|v| v.map(Self::Literal))
                .or_else(|_| Node::parse(parser).map(|v| v.map(Self::Node)))
                .or_else(|_| ArrayExpr::parse(parser).map(|v| v.map(Self::Array)))
                .or_else(|_| TypeIndexExpr::parse(parser).map(|v| v.map(Self::TypeIndex)))
                .or_else(|_| Ident::parse(parser).map(|v| v.map(Self::Ident)))
                .or_else(|_| parser.consume(&Token::This).map(|v| v.wrap(Self::This)))
        }
    }

    fn parse_pratt_expr(parser: &mut Parser, precedence: Precedence) -> ParseResult<Positioned<Self>> {
        let left = Self::parse_atom(parser)?;

        go_parse_pratt_expr(parser, precedence, left)
    }
}

impl Parse for Expression {
    fn parse(parser: &mut Parser) -> ParseResult<Positioned<Self>> {
        Self::parse_pratt_expr(parser, Precedence::PLowest)
    }
}
