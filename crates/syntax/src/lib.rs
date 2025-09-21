pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::{Assignment, BinaryOp, Expr, Statement, UnaryOp, VarDeclaration};
pub use parser::parse_statement;
