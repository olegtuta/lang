use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    Integer,
    Float,
    Bool,
    Str,
    Mixed,
}

impl PrimitiveType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrimitiveType::Integer => "int",
            PrimitiveType::Float => "float",
            PrimitiveType::Bool => "bool",
            PrimitiveType::Str => "str",
            PrimitiveType::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Primitive(PrimitiveType),
}

impl fmt::Display for TypeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeKind::Primitive(p) => write!(f, "{}", p.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LangType {
    kind: TypeKind,
    mutable: bool,
}

impl LangType {
    pub const fn new(kind: TypeKind, mutable: bool) -> Self {
        Self { kind, mutable }
    }

    pub fn integer() -> Self {
        Self::new(TypeKind::Primitive(PrimitiveType::Integer), false)
    }

    pub fn float() -> Self {
        Self::new(TypeKind::Primitive(PrimitiveType::Float), false)
    }

    pub fn boolean() -> Self {
        Self::new(TypeKind::Primitive(PrimitiveType::Bool), false)
    }

    pub fn string() -> Self {
        Self::new(TypeKind::Primitive(PrimitiveType::Str), false)
    }

    pub fn mixed() -> Self {
        Self::new(TypeKind::Primitive(PrimitiveType::Mixed), false)
    }

    pub fn kind(&self) -> &TypeKind {
        &self.kind
    }

    pub fn is_mutable(&self) -> bool {
        self.mutable
    }

    pub fn with_mutability(mut self, mutable: bool) -> Self {
        self.mutable = mutable;
        self
    }
}

impl fmt::Display for LangType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mutable {
            write!(f, "{} mut", self.kind)
        } else {
            write!(f, "{}", self.kind)
        }
    }
}

#[derive(Debug, Default)]
pub struct TypeRegistry {
    types: HashMap<String, TypeKind>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            types: HashMap::new(),
        };
        registry.register_builtin(
            PrimitiveType::Integer.as_str(),
            TypeKind::Primitive(PrimitiveType::Integer),
        );
        registry.register_builtin(
            PrimitiveType::Float.as_str(),
            TypeKind::Primitive(PrimitiveType::Float),
        );
        registry.register_builtin(
            PrimitiveType::Bool.as_str(),
            TypeKind::Primitive(PrimitiveType::Bool),
        );
        registry.register_builtin(
            PrimitiveType::Str.as_str(),
            TypeKind::Primitive(PrimitiveType::Str),
        );
        registry.register_builtin(
            PrimitiveType::Mixed.as_str(),
            TypeKind::Primitive(PrimitiveType::Mixed),
        );
        registry
    }

    pub fn register_builtin(&mut self, name: impl Into<String>, kind: TypeKind) {
        self.types.insert(name.into(), kind);
    }

    pub fn resolve(&self, name: &str) -> Option<TypeKind> {
        self.types.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_type_is_registered() {
        let registry = TypeRegistry::new();
        let ty = registry.resolve("int");
        assert_eq!(ty, Some(TypeKind::Primitive(PrimitiveType::Integer)));
    }

    #[test]
    fn float_type_is_registered() {
        let registry = TypeRegistry::new();
        let ty = registry.resolve("float");
        assert_eq!(ty, Some(TypeKind::Primitive(PrimitiveType::Float)));
    }

    #[test]
    fn bool_type_is_registered() {
        let registry = TypeRegistry::new();
        let ty = registry.resolve("bool");
        assert_eq!(ty, Some(TypeKind::Primitive(PrimitiveType::Bool)));
    }

    #[test]
    fn string_type_is_registered() {
        let registry = TypeRegistry::new();
        let ty = registry.resolve("str");
        assert_eq!(ty, Some(TypeKind::Primitive(PrimitiveType::Str)));
    }

    #[test]
    fn mixed_type_is_registered() {
        let registry = TypeRegistry::new();
        let ty = registry.resolve("mixed");
        assert_eq!(ty, Some(TypeKind::Primitive(PrimitiveType::Mixed)));
    }

    #[test]
    fn lang_type_mutability() {
        let ty = LangType::integer().with_mutability(true);
        assert!(ty.is_mutable());
    }
}
