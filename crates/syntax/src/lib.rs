pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::VarDeclaration;
pub use parser::parse_variable_declaration;
