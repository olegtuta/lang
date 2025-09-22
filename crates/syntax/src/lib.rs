pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod pretty;
pub mod token;

pub use ast::{
    ArrayElement, Assignment, AssignmentKind, AssignmentTarget, BinaryOp, ElseBranch, Expr,
    IfStatement, IncrementOp, IndexTarget, Literal, Statement, TypeAnnotation, UnaryOp,
    VarDeclaration, WhileStatement,
};
pub use error::{SyntaxError, SyntaxResult};
pub use parser::{parse_program, parse_statement};
pub use pretty::format_statement;
