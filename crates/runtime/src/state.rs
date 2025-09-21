use std::collections::HashMap;

use lang_core::{LangError, LangResult, LangType, Value};

#[derive(Debug, Clone)]
pub struct BindingState {
    ty: LangType,
    value: Option<Value>,
}

impl BindingState {
    pub fn new(ty: LangType) -> Self {
        Self { ty, value: None }
    }

    pub fn ty(&self) -> &LangType {
        &self.ty
    }

    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    pub fn assign(&mut self, value: Value) -> LangResult<()> {
        ensure_compatible(&self.ty, &value)?;
        if !self.ty.is_mutable() && self.value.is_some() {
            return Err(LangError::Type(
                "attempt to reassign immutable binding".to_string(),
            ));
        }
        self.value = Some(value);
        Ok(())
    }
}

fn ensure_compatible(expected: &LangType, value: &Value) -> LangResult<()> {
    let value_type = value.ty();
    if value_type.kind() != expected.kind() {
        return Err(LangError::Type(format!(
            "type mismatch: expected {}, got {}",
            expected.kind(),
            value_type.kind(),
        )));
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct Scope {
    bindings: HashMap<String, BindingState>,
}

impl Scope {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn declare(&mut self, name: impl Into<String>, ty: LangType) -> LangResult<()> {
        let name = name.into();
        if self.bindings.contains_key(&name) {
            return Err(LangError::Type(format!(
                "variable `{}` is already declared in scope",
                name
            )));
        }
        self.bindings.insert(name, BindingState::new(ty));
        Ok(())
    }

    pub fn declare_with_value(
        &mut self,
        name: impl Into<String>,
        mut ty: LangType,
        value: Value,
    ) -> LangResult<()> {
        let name = name.into();
        if self.bindings.contains_key(&name) {
            return Err(LangError::Type(format!(
                "variable `{}` is already declared in scope",
                name
            )));
        }
        ensure_compatible(&ty, &value)?;
        let mutable = ty.is_mutable();
        ty = ty.with_mutability(mutable);
        self.bindings.insert(
            name,
            BindingState {
                ty,
                value: Some(value),
            },
        );
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&BindingState> {
        self.bindings.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut BindingState> {
        self.bindings.get_mut(name)
    }

    pub fn assign(&mut self, name: &str, value: Value) -> LangResult<()> {
        let binding = self
            .bindings
            .get_mut(name)
            .ok_or_else(|| LangError::Runtime(format!("variable `{name}` is not defined")))?;
        binding.assign(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declare_integer_binding_without_value() {
        let mut scope = Scope::new();
        scope
            .declare("myVar", LangType::integer())
            .expect("declaration should succeed");
        let binding = scope.get("myVar").unwrap();
        assert!(binding.value().is_none());
    }

    #[test]
    fn declare_integer_binding_with_value() {
        let mut scope = Scope::new();
        scope
            .declare_with_value("myVar", LangType::integer(), Value::from(10))
            .expect("declaration should succeed");
        let binding = scope.get("myVar").unwrap();
        assert_eq!(binding.value().unwrap().expect_integer().unwrap(), 10);
    }

    #[test]
    fn reassigning_immutable_binding_fails() {
        let mut scope = Scope::new();
        scope
            .declare_with_value("counter", LangType::integer(), Value::from(1))
            .unwrap();
        let err = scope
            .get_mut("counter")
            .unwrap()
            .assign(Value::from(2))
            .unwrap_err();
        assert!(format!("{}", err).contains("immutable"));
    }

    #[test]
    fn assigning_mutable_binding_succeeds() {
        let mut scope = Scope::new();
        let ty = LangType::integer().with_mutability(true);
        scope
            .declare_with_value("counter", ty.clone(), Value::from(1))
            .unwrap();
        scope.assign("counter", Value::from(2)).unwrap();
        assert_eq!(
            scope
                .get("counter")
                .unwrap()
                .value()
                .unwrap()
                .expect_integer()
                .unwrap(),
            2
        );
        assert!(scope.get("counter").unwrap().ty().is_mutable());
    }

    #[test]
    fn assigning_undefined_binding_fails() {
        let mut scope = Scope::new();
        let err = scope.assign("missing", Value::from(1)).unwrap_err();
        assert!(format!("{}", err).contains("not defined"));
    }
}
