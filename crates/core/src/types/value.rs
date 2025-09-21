use std::fmt;

use crate::diagnostics::{LangError, LangResult};

use super::registry::{LangType, PrimitiveType, TypeKind};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

impl Value {
    pub fn ty(&self) -> LangType {
        match self {
            Value::Integer(_) => LangType::new(TypeKind::Primitive(PrimitiveType::Integer), false),
            Value::Float(_) => LangType::new(TypeKind::Primitive(PrimitiveType::Float), false),
            Value::Bool(_) => LangType::new(TypeKind::Primitive(PrimitiveType::Bool), false),
            Value::Str(_) => LangType::new(TypeKind::Primitive(PrimitiveType::Str), false),
        }
    }

    pub fn expect_integer(&self) -> LangResult<i64> {
        match self {
            Value::Integer(v) => Ok(*v),
            _ => Err(LangError::Type("expected integer value".to_string())),
        }
    }

    pub fn expect_float(&self) -> LangResult<f64> {
        match self {
            Value::Float(v) => Ok(*v),
            _ => Err(LangError::Type("expected float value".to_string())),
        }
    }

    pub fn expect_bool(&self) -> LangResult<bool> {
        match self {
            Value::Bool(v) => Ok(*v),
            _ => Err(LangError::Type("expected bool value".to_string())),
        }
    }

    pub fn expect_string(&self) -> LangResult<&str> {
        match self {
            Value::Str(v) => Ok(v),
            _ => Err(LangError::Type("expected string value".to_string())),
        }
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Integer(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Str(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Str(value.to_string())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Str(v) => write!(f, "{}", v),
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

    #[test]
    fn float_value_roundtrip() {
        let value = Value::from(3.14_f64);
        assert!((value.expect_float().unwrap() - 3.14).abs() < f64::EPSILON);
        assert_eq!(value.ty(), LangType::float());
    }

    #[test]
    fn bool_value_roundtrip() {
        let value = Value::from(true);
        assert!(value.expect_bool().unwrap());
        assert_eq!(value.ty(), LangType::boolean());
    }

    #[test]
    fn string_value_roundtrip() {
        let value = Value::from("hello");
        assert_eq!(value.expect_string().unwrap(), "hello");
        assert_eq!(value.ty(), LangType::string());
    }
}
