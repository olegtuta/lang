pub mod codegen;
pub mod diagnostics;
pub mod hir;
pub mod mir;
pub mod resolve;
pub mod types;

pub use diagnostics::{LangError, LangResult};
pub use hir::Interpreter;
pub use resolve::{BindingState, Scope};
pub use types::{LangType, PrimitiveType, TypeKind, TypeRegistry, Value};
