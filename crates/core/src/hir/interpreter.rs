use lang_syntax::ast::{
    Assignment, AssignmentKind, BinaryOp, Expr, IncrementOp, Literal, Statement, TypeAnnotation,
    UnaryOp, VarDeclaration,
};

use crate::diagnostics::{LangError, LangResult};
use crate::resolve::Scope;
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

    pub fn execute(&mut self, statement: Statement) -> LangResult<Option<Value>> {
        match statement {
            Statement::Let(decl) => {
                self.execute_var_declaration(decl)?;
                Ok(None)
            }
            Statement::Assignment(assign) => {
                self.execute_assignment(assign)?;
                Ok(None)
            }
            Statement::Echo(expr) => {
                let value = self.evaluate_expr(&expr)?;
                Ok(Some(value))
            }
        }
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
        let Assignment { name, kind } = assignment;
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
                    (_, _) => {
                        return Err(LangError::Type(format!(
                            "`{}` can only be applied to numeric values",
                            match op {
                                IncrementOp::Increment => "++",
                                IncrementOp::Decrement => "--",
                            }
                        )));
                    }
                };
                self.scope.assign(&name, updated)
            }
        }
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

    fn resolve_annotation(&self, annotation: Option<&TypeAnnotation>) -> LangResult<LangType> {
        let kind = match annotation {
            Some(annotation) => self
                .registry
                .resolve(&annotation.name)
                .ok_or_else(|| LangError::unknown_type(&annotation.name))?,
            None => self
                .registry
                .resolve("mixed")
                .expect("mixed type must be registered"),
        };
        Ok(LangType::new(kind, false))
    }

    fn literal_to_value(&self, literal: &Literal) -> LangResult<Value> {
        Ok(match literal {
            Literal::Integer(v) => Value::Integer(*v),
            Literal::Float(v) => Value::Float(*v),
            Literal::Bool(v) => Value::Bool(*v),
            Literal::Str(v) => Value::Str(v.clone()),
        })
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
}

#[cfg(test)]
mod tests {
    use lang_syntax::ast::{
        Assignment, AssignmentKind, BinaryOp, Expr, IncrementOp, Literal, Statement,
        TypeAnnotation, UnaryOp, VarDeclaration,
    };

    use super::Interpreter;

    fn decl(name: &str, ty: Option<&str>, mutable: bool, value: Option<Expr>) -> Statement {
        let annotation = ty.map(|t| TypeAnnotation::new(t.to_string()));
        Statement::Let(VarDeclaration::new(
            name.to_string(),
            annotation,
            mutable,
            value,
        ))
    }

    fn assign(name: &str, expr: Expr) -> Statement {
        Statement::Assignment(Assignment::new(
            name.to_string(),
            AssignmentKind::Simple(expr),
        ))
    }

    fn compound(name: &str, op: BinaryOp, expr: Expr) -> Statement {
        Statement::Assignment(Assignment::new(
            name.to_string(),
            AssignmentKind::Compound { op, expr },
        ))
    }

    fn increment(name: &str, op: IncrementOp) -> Statement {
        Statement::Assignment(Assignment::new(
            name.to_string(),
            AssignmentKind::Increment(op),
        ))
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
        assert_eq!(result.unwrap().expect_integer().unwrap(), 5);
    }

    #[test]
    fn reassign_mutable_variable() {
        let mut interpreter = Interpreter::new();
        interpreter
            .execute(decl(
                "counter",
                Some("int"),
                true,
                Some(Expr::Literal(Literal::Integer(1))),
            ))
            .unwrap();
        interpreter
            .execute(assign(
                "counter",
                Expr::Binary {
                    left: Box::new(Expr::Variable("counter".to_string())),
                    op: BinaryOp::Add,
                    right: Box::new(Expr::Literal(Literal::Integer(1))),
                },
            ))
            .unwrap();
        let binding = interpreter.scope().get("counter").unwrap();
        assert_eq!(binding.value().unwrap().expect_integer().unwrap(), 2);
    }

    #[test]
    fn arithmetic_and_comparison() {
        let mut interpreter = Interpreter::new();
        interpreter
            .execute(decl(
                "threshold",
                Some("float"),
                false,
                Some(Expr::Literal(Literal::Float(2.4))),
            ))
            .unwrap();
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal(Literal::Float(5.0))),
            op: BinaryOp::Divide,
            right: Box::new(Expr::Literal(Literal::Float(2.0))),
        };
        let comparison = Expr::Binary {
            left: Box::new(expr),
            op: BinaryOp::Greater,
            right: Box::new(Expr::Variable("threshold".to_string())),
        };
        let result = interpreter.execute(echo(comparison)).unwrap();
        assert!(result.unwrap().expect_bool().unwrap());
    }

    #[test]
    fn logical_operations() {
        let mut interpreter = Interpreter::new();
        let condition = Expr::Binary {
            left: Box::new(Expr::Literal(Literal::Bool(true))),
            op: BinaryOp::And,
            right: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal(Literal::Integer(3))),
                op: BinaryOp::Less,
                right: Box::new(Expr::Literal(Literal::Integer(5))),
            }),
        };
        let result = interpreter.execute(echo(condition)).unwrap();
        assert!(result.unwrap().expect_bool().unwrap());
    }

    #[test]
    fn unary_negation() {
        let mut interpreter = Interpreter::new();
        let expr = Expr::Unary {
            op: UnaryOp::Negate,
            expr: Box::new(Expr::Literal(Literal::Integer(5))),
        };
        let result = interpreter.execute(echo(expr)).unwrap();
        assert_eq!(result.unwrap().expect_integer().unwrap(), -5);
    }

    #[test]
    fn string_concatenation() {
        let mut interpreter = Interpreter::new();
        interpreter
            .execute(decl(
                "greeting",
                Some("str"),
                true,
                Some(Expr::Literal(Literal::Str("hi".to_string()))),
            ))
            .unwrap();
        interpreter
            .execute(assign(
                "greeting",
                Expr::Binary {
                    left: Box::new(Expr::Variable("greeting".to_string())),
                    op: BinaryOp::Add,
                    right: Box::new(Expr::Literal(Literal::Str(" there".to_string()))),
                },
            ))
            .unwrap();
        let value = interpreter
            .scope()
            .get("greeting")
            .unwrap()
            .value()
            .unwrap()
            .expect_string()
            .unwrap()
            .to_string();
        assert_eq!(value, "hi there");
    }

    #[test]
    fn compound_assignment_updates_value() {
        let mut interpreter = Interpreter::new();
        interpreter
            .execute(decl(
                "total",
                Some("int"),
                true,
                Some(Expr::Literal(Literal::Integer(10))),
            ))
            .unwrap();
        interpreter
            .execute(compound(
                "total",
                BinaryOp::Add,
                Expr::Literal(Literal::Integer(5)),
            ))
            .unwrap();
        let value = interpreter
            .scope()
            .get("total")
            .unwrap()
            .value()
            .unwrap()
            .expect_integer()
            .unwrap();
        assert_eq!(value, 15);
    }

    #[test]
    fn increment_operator_increases_integer() {
        let mut interpreter = Interpreter::new();
        interpreter
            .execute(decl(
                "idx",
                Some("int"),
                true,
                Some(Expr::Literal(Literal::Integer(3))),
            ))
            .unwrap();
        interpreter
            .execute(increment("idx", IncrementOp::Increment))
            .unwrap();
        let value = interpreter
            .scope()
            .get("idx")
            .unwrap()
            .value()
            .unwrap()
            .expect_integer()
            .unwrap();
        assert_eq!(value, 4);
    }

    #[test]
    fn mixed_variable_accepts_multiple_types() {
        let mut interpreter = Interpreter::new();
        interpreter
            .execute(decl("value", None, true, None))
            .unwrap();
        interpreter
            .execute(assign("value", Expr::Literal(Literal::Integer(42))))
            .unwrap();
        interpreter
            .execute(assign(
                "value",
                Expr::Literal(Literal::Str("done".to_string())),
            ))
            .unwrap();
        let stored = interpreter
            .scope()
            .get("value")
            .unwrap()
            .value()
            .unwrap()
            .expect_string()
            .unwrap();
        assert_eq!(stored, "done");
    }
}
