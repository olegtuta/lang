use lang_core::{LangType, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct VarDeclaration {
    pub name: String,
    pub ty: LangType,
    pub value: Option<Value>,
}

impl VarDeclaration {
    pub fn new(name: impl Into<String>, ty: LangType, value: Option<Value>) -> Self {
        Self {
            name: name.into(),
            ty,
            value,
        }
    }
}
