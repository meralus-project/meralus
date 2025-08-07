mod array;
mod as_expr;
mod binary;
mod block;
mod call;
mod enum_path;
mod ident;
mod if_else;
mod index;
mod literal;
mod loop_expr;
mod node;
mod type_index;
mod while_expr;

use mollie_lexer::Token;
use mollie_shared::{Operator, Positioned};

pub use self::{
    array::ArrayExpr,
    as_expr::{AsExpr, AsPattern, NameValuePattern},
    binary::BinaryExpression,
    block::{Block, parse_statements_until},
    call::FunctionCall,
    enum_path::EnumPathExpr,
    ident::Ident,
    if_else::IfElseExpression,
    index::{IndexExpr, IndexTarget},
    literal::{Literal, Number, SizeType},
    node::{NameValue, Node},
    type_index::TypeIndexExpr,
    while_expr::WhileExpression,
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
    const fn from_ref(token: &Token) -> (Self, Option<Operator>) {
        match &token {
            Token::AndAnd => (Self::PAnd, Some(Operator::And)),
            Token::OrOr => (Self::POr, Some(Operator::Or)),
            Token::Eq => (Self::PEquals, Some(Operator::Assign)),
            Token::EqEq => (Self::PEquals, Some(Operator::Equal)),
            Token::NotEq => (Self::PEquals, Some(Operator::NotEqual)),
            Token::Less => (Self::PLessGreater, Some(Operator::LessThan)),
            Token::Greater => (Self::PLessGreater, Some(Operator::GreaterThan)),
            Token::Plus => (Self::PSum, Some(Operator::Add)),
            Token::Minus => (Self::PSum, Some(Operator::Sub)),
            Token::Star => (Self::PProduct, Some(Operator::Mul)),
            Token::Slash => (Self::PProduct, Some(Operator::Div)),
            Token::ParenOpen => (Self::PCall, None),
            Token::BracketOpen | Token::Dot => (Self::PIndex, None),
            Token::Is => (Self::PCheck, None),
            _ => (Self::PLowest, None),
        }
    }
}

fn go_parse_pratt_expr(parser: &mut Parser, precedence: Precedence, left: Positioned<Expression>) -> ParseResult<Positioned<Expression>> {
    if let Some(value) = parser.peek() {
        let (p, _) = Precedence::from_ref(&value.value);

        match p {
            Precedence::PCheck if precedence < Precedence::PCheck => {
                parser.consume(&Token::Is)?;

                let pattern = AsPattern::parse(parser)?;

                let left = left.between(&pattern).wrap(Expression::AsExpr(AsExpr {
                    target: Box::new(left),
                    pattern,
                }));

                go_parse_pratt_expr(parser, precedence, left)
            }
            Precedence::PCall if precedence < Precedence::PCall => {
                let left = FunctionCall::parse(parser, left)?.map(Expression::FunctionCall);

                go_parse_pratt_expr(parser, precedence, left)
            }
            Precedence::PIndex if precedence < Precedence::PIndex => {
                let left = IndexExpr::parse(parser, left)?.map(Expression::Index);

                go_parse_pratt_expr(parser, precedence, left)
            }
            ref peek_precedence if precedence < *peek_precedence => {
                let left = BinaryExpression::parse(parser, left)?.map(Expression::Binary);

                go_parse_pratt_expr(parser, precedence, left)
            }
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
    Binary(BinaryExpression),
    TypeIndex(TypeIndexExpr),
    Array(ArrayExpr),
    IfElse(IfElseExpression),
    While(WhileExpression),
    Block(Block),
    EnumPath(EnumPathExpr),
    AsExpr(AsExpr),
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
                .or_else(|_| Block::parse(parser).map(|v| v.map(Self::Block)))
                .or_else(|_| IfElseExpression::parse(parser).map(|v| v.map(Self::IfElse)))
                .or_else(|_| WhileExpression::parse(parser).map(|v| v.map(Self::While)))
                .or_else(|_| ArrayExpr::parse(parser).map(|v| v.map(Self::Array)))
                .or_else(|_| TypeIndexExpr::parse(parser).map(|v| v.map(Self::TypeIndex)))
                .or_else(|_| EnumPathExpr::parse(parser).map(|v| v.map(Self::EnumPath)))
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
