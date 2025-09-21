use std::fmt;

use crate::data::types::{LangType, PrimitiveType, TypeKind};
use crate::diagnostics::LangResult;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
}

impl Value {
    pub fn ty(&self) -> LangType {
        match self {
            Value::Integer(_) => LangType::new(TypeKind::Primitive(PrimitiveType::Integer), false),
        }
    }

    pub fn expect_integer(&self) -> LangResult<i64> {
        match self {
            Value::Integer(v) => Ok(*v),
        }
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Integer(value)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(v) => write!(f, "{}", v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_value_roundtrip() {
        let value = Value::from(42);
        assert_eq!(value.expect_integer().unwrap(), 42);
        assert_eq!(value.ty(), LangType::integer());
    }
}
