use std::fmt;

use crate::diagnostics::{LangError, LangResult};

use super::registry::{LangType, PrimitiveType, TypeKind};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(ArrayValue),
}

impl Value {
    pub fn ty(&self) -> LangType {
        match self {
            Value::Integer(_) => LangType::new(TypeKind::Primitive(PrimitiveType::Integer), false),
            Value::Float(_) => LangType::new(TypeKind::Primitive(PrimitiveType::Float), false),
            Value::Bool(_) => LangType::new(TypeKind::Primitive(PrimitiveType::Bool), false),
            Value::Str(_) => LangType::new(TypeKind::Primitive(PrimitiveType::Str), false),
            Value::Array(_) => LangType::array(TypeKind::Primitive(PrimitiveType::Mixed)),
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

    pub fn expect_array(&self) -> LangResult<&ArrayValue> {
        match self {
            Value::Array(array) => Ok(array),
            _ => Err(LangError::Type("expected array value".to_string())),
        }
    }

    pub fn expect_array_mut(&mut self) -> LangResult<&mut ArrayValue> {
        match self {
            Value::Array(array) => Ok(array),
            _ => Err(LangError::Type("expected array value".to_string())),
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

impl From<ArrayValue> for Value {
    fn from(value: ArrayValue) -> Self {
        Value::Array(value)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Str(v) => write!(f, "{}", v),
            Value::Array(array) => write!(f, "{}", array),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayValue {
    entries: Vec<(ArrayKey, Value)>,
    next_index: i64,
}

impl ArrayValue {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_index: 0,
        }
    }

    pub fn push(&mut self, value: Value) -> ArrayKey {
        let key = ArrayKey::Int(self.next_index);
        self.next_index += 1;
        self.entries.push((key.clone(), value));
        key
    }

    pub fn insert(&mut self, key: ArrayKey, value: Value) {
        if let Some((_, existing)) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            *existing = value;
        } else {
            self.entries.push((key.clone(), value));
        }
        if let ArrayKey::Int(index) = key {
            if index >= self.next_index {
                self.next_index = index + 1;
            }
        }
    }

    pub fn get(&self, key: &ArrayKey) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &ArrayKey) -> Option<&mut Value> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ArrayKey, &Value)> + '_ {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

impl fmt::Display for ArrayValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        write!(f, "[")?;
        for (key, value) in self.iter() {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{} => {}", key, value)?;
        }
        write!(f, "]")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArrayKey {
    Int(i64),
    Str(String),
}

impl ArrayKey {
    pub fn from_value(value: Value) -> LangResult<Self> {
        match value {
            Value::Integer(v) => Ok(ArrayKey::Int(v)),
            Value::Str(v) => Ok(ArrayKey::Str(v)),
            other => Err(LangError::Type(format!(
                "array indices must be integers or strings, got {}",
                other.ty().kind()
            ))),
        }
    }
}

impl fmt::Display for ArrayKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArrayKey::Int(index) => write!(f, "{}", index),
            ArrayKey::Str(name) => write!(f, "'{}'", name),
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

    #[test]
    fn array_value_roundtrip() {
        let mut array = ArrayValue::new();
        array.push(Value::from(1));
        array.insert(ArrayKey::Str("key".to_string()), Value::from("value"));
        let value = Value::from(array.clone());
        assert_eq!(value.expect_array().unwrap().iter().count(), 2);
        assert!(value.ty().as_array().is_some());
        assert_eq!(
            format!("{}", Value::from(array)),
            "[0 => 1, 'key' => value]"
        );
    }
}
