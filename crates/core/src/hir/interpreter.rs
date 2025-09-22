use lang_syntax::ast::{
    ArrayElement, Assignment, AssignmentKind, AssignmentTarget, BinaryOp, ElseBranch, Expr,
    IfStatement, IncrementOp, IndexTarget, Literal, Statement, TypeAnnotation, UnaryOp,
    VarDeclaration, WhileStatement,
};

use crate::diagnostics::{LangError, LangResult};
use crate::resolve::Scope;
use crate::types::registry::{ArrayType, PrimitiveType, TypeKind};
use crate::types::value::{ArrayKey, ArrayValue};
use crate::types::{LangType, TypeRegistry, Value};

pub struct Interpreter {
    scope: Scope,
    registry: TypeRegistry,
}

impl Interpreter {
    pub fn new() -> Self {
        Self::with_registry(TypeRegistry::new())
    }

    pub fn with_registry(registry: TypeRegistry) -> Self {
        Self {
            scope: Scope::new(),
            registry,
        }
    }

    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    pub fn registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.registry
    }

    pub fn execute(&mut self, statement: Statement) -> LangResult<Vec<Value>> {
        let mut output = Vec::new();
        let flow = self.execute_statement(&statement, &mut output)?;
        match flow {
            ControlFlow::None => Ok(output),
            ControlFlow::Break => Err(LangError::Runtime("`break` outside of loop".to_string())),
            ControlFlow::Continue => {
                Err(LangError::Runtime("`continue` outside of loop".to_string()))
            }
        }
    }

    fn execute_statement(
        &mut self,
        statement: &Statement,
        output: &mut Vec<Value>,
    ) -> LangResult<ControlFlow> {
        match statement {
            Statement::Let(decl) => {
                self.execute_var_declaration(decl.clone())?;
                Ok(ControlFlow::None)
            }
            Statement::Assignment(assign) => {
                self.execute_assignment(assign.clone())?;
                Ok(ControlFlow::None)
            }
            Statement::Echo(expr) => {
                let value = self.evaluate_expr(expr)?;
                output.push(value);
                Ok(ControlFlow::None)
            }
            Statement::If(if_stmt) => self.execute_if(if_stmt, output),
            Statement::While(while_stmt) => self.execute_while(while_stmt, output),
            Statement::Break => Ok(ControlFlow::Break),
            Statement::Continue => Ok(ControlFlow::Continue),
        }
    }

    fn execute_block(
        &mut self,
        statements: &[Statement],
        output: &mut Vec<Value>,
    ) -> LangResult<ControlFlow> {
        for statement in statements {
            match self.execute_statement(statement, output)? {
                ControlFlow::None => continue,
                flow => return Ok(flow),
            }
        }
        Ok(ControlFlow::None)
    }

    fn execute_if(
        &mut self,
        if_stmt: &IfStatement,
        output: &mut Vec<Value>,
    ) -> LangResult<ControlFlow> {
        let condition = self.evaluate_expr(&if_stmt.condition)?;
        let value = condition
            .expect_bool()
            .map_err(|err| LangError::Type(format!("if condition must be boolean: {err}")))?;
        if value {
            self.execute_block(&if_stmt.then_branch, output)
        } else if let Some(else_branch) = &if_stmt.else_branch {
            match else_branch.as_ref() {
                ElseBranch::If(nested) => self.execute_if(nested, output),
                ElseBranch::Block(block) => self.execute_block(block, output),
            }
        } else {
            Ok(ControlFlow::None)
        }
    }

    fn execute_while(
        &mut self,
        while_stmt: &WhileStatement,
        output: &mut Vec<Value>,
    ) -> LangResult<ControlFlow> {
        loop {
            let condition = self.evaluate_expr(&while_stmt.condition)?;
            let cond = condition.expect_bool().map_err(|err| {
                LangError::Type(format!("while condition must be boolean: {err}"))
            })?;
            if !cond {
                break;
            }
            match self.execute_block(&while_stmt.body, output)? {
                ControlFlow::None => {}
                ControlFlow::Break => break,
                ControlFlow::Continue => continue,
            }
        }
        Ok(ControlFlow::None)
    }

    fn execute_var_declaration(&mut self, decl: VarDeclaration) -> LangResult<()> {
        let VarDeclaration {
            name,
            ty,
            mutable,
            value,
        } = decl;
        let mut lang_type = self.resolve_annotation(ty.as_ref())?;
        lang_type = lang_type.with_mutability(mutable);
        if let Some(expr) = value {
            let evaluated = self.evaluate_expr(&expr)?;
            self.scope
                .declare_with_value(name, lang_type.clone(), evaluated)
                .map(|_| ())
        } else {
            self.scope.declare(name, lang_type)
        }
    }

    fn execute_assignment(&mut self, assignment: Assignment) -> LangResult<()> {
        match assignment.target {
            AssignmentTarget::Name(name) => self.assign_to_name(name, assignment.kind),
            AssignmentTarget::Indexed { name, indices } => {
                self.assign_to_index(name, indices, assignment.kind)
            }
        }
    }

    fn assign_to_name(&mut self, name: String, kind: AssignmentKind) -> LangResult<()> {
        match kind {
            AssignmentKind::Simple(expr) => {
                let evaluated = self.evaluate_expr(&expr)?;
                self.scope.assign(&name, evaluated)
            }
            AssignmentKind::Compound { op, expr } => {
                let rhs = self.evaluate_expr(&expr)?;
                let lhs = self.current_value(&name)?;
                let result = self.evaluate_binary(op, lhs, rhs)?;
                self.scope.assign(&name, result)
            }
            AssignmentKind::Increment(op) => {
                let current = self.current_value(&name)?;
                let updated = match (op, current) {
                    (IncrementOp::Increment, Value::Integer(v)) => Value::Integer(v + 1),
                    (IncrementOp::Decrement, Value::Integer(v)) => Value::Integer(v - 1),
                    (IncrementOp::Increment, Value::Float(v)) => Value::Float(v + 1.0),
                    (IncrementOp::Decrement, Value::Float(v)) => Value::Float(v - 1.0),
                    _ => {
                        return Err(LangError::Type(format!(
                            "`{}` can only be applied to numeric values",
                            match op {
                                IncrementOp::Increment => "++",
                                IncrementOp::Decrement => "--",
                            }
                        )))
                    }
                };
                self.scope.assign(&name, updated)
            }
        }
    }

    fn assign_to_index(
        &mut self,
        name: String,
        indices: Vec<IndexTarget>,
        kind: AssignmentKind,
    ) -> LangResult<()> {
        let AssignmentKind::Simple(expr) = kind else {
            return Err(LangError::Type(
                "only simple assignments are supported for indexed targets".to_string(),
            ));
        };
        let value = self.evaluate_expr(&expr)?;
        let resolved_indices = self.resolve_indices(indices)?;
        let binding_type = self
            .scope
            .get(&name)
            .ok_or_else(|| LangError::Runtime(format!("variable `{name}` is not defined")))?
            .ty()
            .clone();
        let mut current = self.current_value(&name)?;
        self.apply_index_assignment(&binding_type, &mut current, &resolved_indices, value)?;
        self.scope.assign(&name, current)
    }

    fn resolve_indices(&mut self, indices: Vec<IndexTarget>) -> LangResult<Vec<ResolvedIndex>> {
        let mut resolved = Vec::new();
        for index in indices {
            match index {
                IndexTarget::Append => resolved.push(ResolvedIndex::Append),
                IndexTarget::Index(expr) => {
                    let value = self.evaluate_expr(&expr)?;
                    let key = ArrayKey::from_value(value)?;
                    resolved.push(ResolvedIndex::Key(key));
                }
            }
        }
        Ok(resolved)
    }

    fn apply_index_assignment(
        &mut self,
        binding_type: &LangType,
        current: &mut Value,
        indices: &[ResolvedIndex],
        value: Value,
    ) -> LangResult<()> {
        let Some(array_type) = binding_type.as_array() else {
            return Err(LangError::Type(
                "attempt to index into non-array binding".to_string(),
            ));
        };
        let Value::Array(ref mut array) = current else {
            return Err(LangError::Runtime(
                "attempt to index into uninitialized array".to_string(),
            ));
        };
        assign_into_array(array_type, array, indices, value)
    }

    fn current_value(&self, name: &str) -> LangResult<Value> {
        let binding = self
            .scope
            .get(name)
            .ok_or_else(|| LangError::Runtime(format!("variable `{name}` is not defined")))?;
        binding
            .value()
            .cloned()
            .ok_or_else(|| LangError::Runtime(format!("variable `{name}` is uninitialized")))
    }

    fn evaluate_expr(&mut self, expr: &Expr) -> LangResult<Value> {
        match expr {
            Expr::Literal(literal) => self.literal_to_value(literal),
            Expr::Variable(name) => self.current_value(name),
            Expr::Index { target, index } => {
                let base = self.evaluate_expr(target)?;
                let Value::Array(array) = base else {
                    return Err(LangError::Runtime(
                        "attempt to index a non-array value".to_string(),
                    ));
                };
                let key_value = self.evaluate_expr(index)?;
                let key = ArrayKey::from_value(key_value)?;
                array
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| LangError::Runtime(format!("key {key} not found")))
            }
            Expr::Unary { op, expr } => {
                let value = self.evaluate_expr(expr)?;
                self.evaluate_unary(*op, value)
            }
            Expr::Binary { left, op, right } => {
                if *op == BinaryOp::And {
                    return self.evaluate_logical_and(left, right);
                }
                if *op == BinaryOp::Or {
                    return self.evaluate_logical_or(left, right);
                }
                let left_value = self.evaluate_expr(left)?;
                let right_value = self.evaluate_expr(right)?;
                self.evaluate_binary(*op, left_value, right_value)
            }
        }
    }

    fn literal_to_value(&mut self, literal: &Literal) -> LangResult<Value> {
        Ok(match literal {
            Literal::Integer(v) => Value::Integer(*v),
            Literal::Float(v) => Value::Float(*v),
            Literal::Bool(v) => Value::Bool(*v),
            Literal::Str(v) => Value::Str(v.clone()),
            Literal::Array(elements) => self.evaluate_array_literal(elements)?,
        })
    }

    fn evaluate_array_literal(&mut self, elements: &[ArrayElement]) -> LangResult<Value> {
        let mut array = ArrayValue::new();
        for element in elements {
            match element {
                ArrayElement::Value(expr) => {
                    let value = self.evaluate_expr(expr)?;
                    array.push(value);
                }
                ArrayElement::KeyValue { key, value } => {
                    let key_value = self.evaluate_expr(key)?;
                    let array_key = ArrayKey::from_value(key_value)?;
                    let array_value = self.evaluate_expr(value)?;
                    array.insert(array_key, array_value);
                }
            }
        }
        Ok(Value::Array(array))
    }

    fn evaluate_unary(&self, op: UnaryOp, value: Value) -> LangResult<Value> {
        match op {
            UnaryOp::Negate => match value {
                Value::Integer(v) => Ok(Value::Integer(-v)),
                Value::Float(v) => Ok(Value::Float(-v)),
                _ => Err(LangError::Type(
                    "unary `-` is only defined for integers and floats".to_string(),
                )),
            },
            UnaryOp::Not => match value {
                Value::Bool(v) => Ok(Value::Bool(!v)),
                _ => Err(LangError::Type(
                    "unary `!` is only defined for boolean values".to_string(),
                )),
            },
        }
    }

    fn evaluate_logical_and(&mut self, left: &Expr, right: &Expr) -> LangResult<Value> {
        let left_value = self.evaluate_expr(left)?;
        match left_value {
            Value::Bool(false) => Ok(Value::Bool(false)),
            Value::Bool(true) => {
                let right_value = self.evaluate_expr(right)?;
                match right_value {
                    Value::Bool(v) => Ok(Value::Bool(v)),
                    _ => Err(LangError::Type(
                        "logical `&&` expects boolean operands".to_string(),
                    )),
                }
            }
            _ => Err(LangError::Type(
                "logical `&&` expects boolean operands".to_string(),
            )),
        }
    }

    fn evaluate_logical_or(&mut self, left: &Expr, right: &Expr) -> LangResult<Value> {
        let left_value = self.evaluate_expr(left)?;
        match left_value {
            Value::Bool(true) => Ok(Value::Bool(true)),
            Value::Bool(false) => {
                let right_value = self.evaluate_expr(right)?;
                match right_value {
                    Value::Bool(v) => Ok(Value::Bool(v)),
                    _ => Err(LangError::Type(
                        "logical `||` expects boolean operands".to_string(),
                    )),
                }
            }
            _ => Err(LangError::Type(
                "logical `||` expects boolean operands".to_string(),
            )),
        }
    }

    fn evaluate_binary(&self, op: BinaryOp, left: Value, right: Value) -> LangResult<Value> {
        match op {
            BinaryOp::Add => match (left, right) {
                (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a + b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
                (Value::Str(mut a), Value::Str(b)) => {
                    a.push_str(&b);
                    Ok(Value::Str(a))
                }
                _ => Err(LangError::Type(
                    "`+` expects matching numeric types or strings".to_string(),
                )),
            },
            BinaryOp::Subtract => match (left, right) {
                (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a - b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
                _ => Err(LangError::Type(
                    "`-` expects matching numeric types".to_string(),
                )),
            },
            BinaryOp::Multiply => match (left, right) {
                (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a * b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
                _ => Err(LangError::Type(
                    "`*` expects matching numeric types".to_string(),
                )),
            },
            BinaryOp::Divide => match (left, right) {
                (Value::Integer(_), Value::Integer(0)) => {
                    Err(LangError::Runtime("division by zero".to_string()))
                }
                (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a / b)),
                (Value::Float(_), Value::Float(b)) if b.abs() < f64::EPSILON => {
                    Err(LangError::Runtime("division by zero".to_string()))
                }
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
                _ => Err(LangError::Type(
                    "`/` expects matching numeric types".to_string(),
                )),
            },
            BinaryOp::Modulo => match (left, right) {
                (Value::Integer(_), Value::Integer(0)) => {
                    Err(LangError::Runtime("modulo by zero".to_string()))
                }
                (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a % b)),
                _ => Err(LangError::Type("`%` expects integer operands".to_string())),
            },
            BinaryOp::Equal => self.compare_equality(left, right, true),
            BinaryOp::NotEqual => self.compare_equality(left, right, false),
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                self.compare_ordering(op, left, right)
            }
            BinaryOp::And | BinaryOp::Or => unreachable!("logical ops handled earlier"),
        }
    }

    fn compare_equality(&self, left: Value, right: Value, expect_equal: bool) -> LangResult<Value> {
        let result = match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            _ => {
                return Err(LangError::Type(
                    "equality comparison requires matching operand types".to_string(),
                ))
            }
        };
        let value = if expect_equal { result } else { !result };
        Ok(Value::Bool(value))
    }

    fn compare_ordering(&self, op: BinaryOp, left: Value, right: Value) -> LangResult<Value> {
        let result = match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => Self::compare_numbers(op, a as f64, b as f64),
            (Value::Float(a), Value::Float(b)) => Self::compare_numbers(op, a, b),
            _ => {
                return Err(LangError::Type(
                    "ordering comparison expects numeric operands".to_string(),
                ))
            }
        }?;
        Ok(Value::Bool(result))
    }

    fn compare_numbers(op: BinaryOp, left: f64, right: f64) -> LangResult<bool> {
        let result = match op {
            BinaryOp::Less => left < right,
            BinaryOp::LessEqual => left <= right,
            BinaryOp::Greater => left > right,
            BinaryOp::GreaterEqual => left >= right,
            _ => unreachable!("unexpected operator in numeric comparison"),
        };
        Ok(result)
    }

    fn resolve_annotation(&self, annotation: Option<&TypeAnnotation>) -> LangResult<LangType> {
        match annotation {
            Some(annotation) => self.resolve_annotation_inner(annotation),
            None => Ok(LangType::mixed()),
        }
    }

    fn resolve_annotation_inner(&self, annotation: &TypeAnnotation) -> LangResult<LangType> {
        if annotation.name == "arr" {
            let (key_ty, value_ty) = match annotation.generics.as_slice() {
                [] => (
                    TypeKind::Primitive(PrimitiveType::Mixed),
                    TypeKind::Primitive(PrimitiveType::Mixed),
                ),
                [value] => (
                    TypeKind::Primitive(PrimitiveType::Mixed),
                    self.resolve_annotation_inner(value)?.kind().clone(),
                ),
                [key, value, ..] => (
                    self.resolve_annotation_inner(key)?.kind().clone(),
                    self.resolve_annotation_inner(value)?.kind().clone(),
                ),
            };
            Ok(LangType::array_with_key(key_ty, value_ty))
        } else {
            if !annotation.generics.is_empty() {
                return Err(LangError::Type(format!(
                    "type `{}` does not accept generic arguments",
                    annotation.name
                )));
            }
            let kind = self
                .registry
                .resolve(&annotation.name)
                .ok_or_else(|| LangError::unknown_type(&annotation.name))?;
            Ok(LangType::new(kind, false))
        }
    }
}

fn assign_into_array(
    array_type: &ArrayType,
    array: &mut ArrayValue,
    indices: &[ResolvedIndex],
    value: Value,
) -> LangResult<()> {
    if indices.is_empty() {
        return Err(LangError::Runtime(
            "missing index for array assignment".to_string(),
        ));
    }
    match &indices[0] {
        ResolvedIndex::Append => {
            if indices.len() != 1 {
                return Err(LangError::Runtime(
                    "`[]` append syntax can only appear at the end of the index chain".to_string(),
                ));
            }
            array.push(value);
            Ok(())
        }
        ResolvedIndex::Key(key) => {
            if indices.len() == 1 {
                array.insert(key.clone(), value);
                Ok(())
            } else {
                let entry = array.get_mut(key).ok_or_else(|| {
                    LangError::Runtime(format!("key {key} does not exist for assignment"))
                })?;
                match entry {
                    Value::Array(inner) => {
                        let next_type = match array_type.value() {
                            TypeKind::Array(inner_type) => inner_type.clone(),
                            TypeKind::Primitive(PrimitiveType::Mixed) => ArrayType::new(
                                TypeKind::Primitive(PrimitiveType::Mixed),
                                TypeKind::Primitive(PrimitiveType::Mixed),
                            ),
                            other => {
                                return Err(LangError::Type(format!(
                                    "cannot index into value of type {}",
                                    other
                                )))
                            }
                        };
                        assign_into_array(&next_type, inner, &indices[1..], value)
                    }
                    _ => Err(LangError::Runtime(
                        "cannot index into non-array value".to_string(),
                    )),
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
enum ResolvedIndex {
    Key(ArrayKey),
    Append,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlFlow {
    None,
    Break,
    Continue,
}

#[cfg(test)]
mod tests {
    use lang_syntax::ast::{
        ArrayElement, Assignment, AssignmentKind, AssignmentTarget, BinaryOp, ElseBranch, Expr,
        IfStatement, IndexTarget, Literal, Statement, TypeAnnotation, VarDeclaration,
        WhileStatement,
    };

    use super::Interpreter;

    fn decl(name: &str, ty: Option<&str>, mutable: bool, value: Option<Expr>) -> Statement {
        let annotation = ty.map(parse_type);
        Statement::Let(VarDeclaration::new(
            name.to_string(),
            annotation,
            mutable,
            value,
        ))
    }

    fn parse_type(spec: &str) -> TypeAnnotation {
        if let Some(start) = spec.find('<') {
            let end = spec
                .rfind('>')
                .expect("generic type must close with `>` in tests");
            let base = &spec[..start];
            let inner = &spec[start + 1..end];
            let generics = if inner.trim().is_empty() {
                Vec::new()
            } else {
                inner
                    .split(',')
                    .map(|part| parse_type(part.trim()))
                    .collect()
            };
            TypeAnnotation::with_generics(base.to_string(), generics)
        } else {
            TypeAnnotation::new(spec.to_string())
        }
    }

    fn echo(expr: Expr) -> Statement {
        Statement::Echo(expr)
    }

    #[test]
    fn declare_and_use_integer() {
        let mut interpreter = Interpreter::new();
        interpreter
            .execute(decl(
                "value",
                Some("int"),
                false,
                Some(Expr::Literal(Literal::Integer(5))),
            ))
            .unwrap();
        let result = interpreter
            .execute(echo(Expr::Variable("value".to_string())))
            .unwrap();
        assert_eq!(result[0].expect_integer().unwrap(), 5);
    }

    #[test]
    fn assign_into_array_literal() {
        let mut interpreter = Interpreter::new();
        interpreter
            .execute(decl(
                "arr",
                Some("arr<int>"),
                true,
                Some(Expr::Literal(Literal::Array(vec![ArrayElement::Value(
                    Expr::Literal(Literal::Integer(1)),
                )]))),
            ))
            .unwrap();
        interpreter
            .execute(Statement::Assignment(Assignment::new(
                AssignmentTarget::Indexed {
                    name: "arr".to_string(),
                    indices: vec![IndexTarget::Append],
                },
                AssignmentKind::Simple(Expr::Literal(Literal::Integer(2))),
            )))
            .unwrap();
        let result = interpreter
            .execute(Statement::Echo(Expr::Index {
                target: Box::new(Expr::Variable("arr".to_string())),
                index: Box::new(Expr::Literal(Literal::Integer(1))),
            }))
            .unwrap();
        assert_eq!(result[0].expect_integer().unwrap(), 2);
    }

    #[test]
    fn if_statement_executes_branch() {
        let mut interpreter = Interpreter::new();
        let condition = Expr::Literal(Literal::Bool(true));
        let if_stmt = Statement::If(IfStatement::new(
            condition,
            vec![Statement::Echo(Expr::Literal(Literal::Integer(1)))],
            Some(Box::new(ElseBranch::Block(vec![Statement::Echo(
                Expr::Literal(Literal::Integer(2)),
            )]))),
        ));
        let result = interpreter.execute(if_stmt).unwrap();
        assert_eq!(result[0].expect_integer().unwrap(), 1);
    }

    #[test]
    fn while_loop_honors_break() {
        let mut interpreter = Interpreter::new();
        interpreter
            .execute(decl(
                "i",
                Some("int"),
                true,
                Some(Expr::Literal(Literal::Integer(0))),
            ))
            .unwrap();
        let loop_stmt = Statement::While(WhileStatement::new(
            Expr::Literal(Literal::Bool(true)),
            vec![
                Statement::Echo(Expr::Variable("i".to_string())),
                Statement::Assignment(Assignment::new(
                    AssignmentTarget::Name("i".to_string()),
                    AssignmentKind::Compound {
                        op: BinaryOp::Add,
                        expr: Expr::Literal(Literal::Integer(1)),
                    },
                )),
                Statement::If(IfStatement::new(
                    Expr::Binary {
                        left: Box::new(Expr::Variable("i".to_string())),
                        op: BinaryOp::Greater,
                        right: Box::new(Expr::Literal(Literal::Integer(1))),
                    },
                    vec![Statement::Break],
                    None,
                )),
            ],
        ));
        let result = interpreter.execute(loop_stmt).unwrap();
        assert_eq!(result.len(), 2);
    }
}
