pub mod data;
pub mod diagnostics;
pub mod task;

pub use data::types::{LangType, PrimitiveType, TypeKind, TypeRegistry};
pub use data::value::Value;
pub use diagnostics::{LangError, LangResult};
