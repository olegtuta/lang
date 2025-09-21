pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod pretty;
pub mod token;

pub use ast::{
    Assignment, AssignmentKind, BinaryOp, Expr, IncrementOp, Literal, Statement, TypeAnnotation,
    UnaryOp, VarDeclaration,
};
pub use error::{SyntaxError, SyntaxResult};
pub use parser::parse_statement;
pub use pretty::format_statement;
