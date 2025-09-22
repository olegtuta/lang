//! Mid-level intermediate representation used for native code generation.

use std::collections::HashMap;

use lang_syntax::ast::{
    Assignment, AssignmentKind, AssignmentTarget, BinaryOp, ElseBranch, Expr, IfStatement,
    IncrementOp, Literal, Statement, TypeAnnotation, UnaryOp, VarDeclaration, WhileStatement,
};

use crate::diagnostics::{LangError, LangResult};

/// Identifier of a lowered variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarId(pub usize);

/// Primitive types supported by the MIR backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirType {
    Int,
    Bool,
}

impl MirType {
    fn expect(&self, other: MirType, context: &str) -> LangResult<()> {
        if *self != other {
            Err(LangError::Type(format!(
                "type mismatch: expected {self:?}, got {other:?} while lowering {context}"
            )))
        } else {
            Ok(())
        }
    }
}

/// Metadata describing a declared variable.
#[derive(Debug, Clone)]
pub struct MirVariable {
    pub name: String,
    pub ty: MirType,
    pub mutable: bool,
}

/// Fully lowered program ready for machine code generation.
#[derive(Debug, Clone)]
pub struct MirProgram {
    pub variables: Vec<MirVariable>,
    pub body: Vec<MirStatement>,
}

/// Statements in the MIR.
#[derive(Debug, Clone)]
pub enum MirStatement {
    Let {
        var: VarId,
        init: Option<MirExpr>,
    },
    Assign {
        var: VarId,
        value: MirExpr,
    },
    Echo(MirExpr),
    If {
        cond: MirExpr,
        then_body: Vec<MirStatement>,
        else_body: Vec<MirStatement>,
    },
    While {
        cond: MirExpr,
        body: Vec<MirStatement>,
    },
    Break,
    Continue,
}

/// Expressions lowered into MIR form.
#[derive(Debug, Clone)]
pub struct MirExpr {
    pub ty: MirType,
    pub kind: MirExprKind,
}

impl MirExpr {
    fn new(ty: MirType, kind: MirExprKind) -> Self {
        Self { ty, kind }
    }

    fn boolean(value: bool) -> Self {
        Self::new(MirType::Bool, MirExprKind::Bool(value))
    }

    fn integer(value: i64) -> Self {
        Self::new(MirType::Int, MirExprKind::Int(value))
    }

    fn variable(var: VarId, ty: MirType) -> Self {
        Self::new(ty, MirExprKind::Var(var))
    }

    fn unary(ty: MirType, op: MirUnaryOp, expr: MirExpr) -> Self {
        Self::new(
            ty,
            MirExprKind::Unary {
                op,
                expr: Box::new(expr),
            },
        )
    }

    fn binary(op: MirBinaryOp, left: MirExpr, right: MirExpr, ty: MirType) -> Self {
        Self::new(
            ty,
            MirExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
        )
    }
}

/// Unary operations supported by MIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirUnaryOp {
    Negate,
    Not,
}

/// Binary operations supported by MIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

/// Fully lowered expression variants.
#[derive(Debug, Clone)]
pub enum MirExprKind {
    Int(i64),
    Bool(bool),
    Var(VarId),
    Unary {
        op: MirUnaryOp,
        expr: Box<MirExpr>,
    },
    Binary {
        op: MirBinaryOp,
        left: Box<MirExpr>,
        right: Box<MirExpr>,
    },
}

/// Lower the parsed AST statements into MIR suitable for native codegen.
pub fn lower_to_mir(statements: &[Statement]) -> LangResult<MirProgram> {
    let mut ctx = LoweringContext::new();
    let mut body = Vec::new();
    for statement in statements {
        body.push(ctx.lower_statement(statement)?);
    }
    Ok(MirProgram {
        variables: ctx.variables,
        body,
    })
}

struct LoweringContext {
    variables: Vec<MirVariable>,
    names: HashMap<String, VarId>,
    loop_depth: usize,
}

impl LoweringContext {
    fn new() -> Self {
        Self {
            variables: Vec::new(),
            names: HashMap::new(),
            loop_depth: 0,
        }
    }

    fn lower_statement(&mut self, statement: &Statement) -> LangResult<MirStatement> {
        match statement {
            Statement::Let(decl) => self.lower_let(decl),
            Statement::Assignment(assign) => self.lower_assignment(assign),
            Statement::Echo(expr) => Ok(MirStatement::Echo(self.lower_expr(expr)?)),
            Statement::If(if_stmt) => self.lower_if(if_stmt),
            Statement::While(while_stmt) => self.lower_while(while_stmt),
            Statement::Break => {
                if self.loop_depth == 0 {
                    return Err(LangError::Runtime("`break` outside of loop".to_string()));
                }
                Ok(MirStatement::Break)
            }
            Statement::Continue => {
                if self.loop_depth == 0 {
                    return Err(LangError::Runtime("`continue` outside of loop".to_string()));
                }
                Ok(MirStatement::Continue)
            }
        }
    }

    fn lower_block(&mut self, statements: &[Statement]) -> LangResult<Vec<MirStatement>> {
        statements
            .iter()
            .map(|stmt| self.lower_statement(stmt))
            .collect()
    }

    fn lower_let(&mut self, decl: &VarDeclaration) -> LangResult<MirStatement> {
        if self.names.contains_key(&decl.name) {
            return Err(LangError::Type(format!(
                "variable `{}` declared more than once",
                decl.name
            )));
        }
        let ty = self.resolve_type(decl.ty.as_ref())?;
        let var_id = self.push_variable(decl.name.clone(), ty, decl.mutable);
        let init = match &decl.value {
            Some(expr) => {
                let lowered = self.lower_expr(expr)?;
                ty.expect(lowered.ty, "variable initializer")?;
                Some(lowered)
            }
            None => None,
        };
        Ok(MirStatement::Let { var: var_id, init })
    }

    fn lower_assignment(&mut self, assignment: &Assignment) -> LangResult<MirStatement> {
        let target = match &assignment.target {
            AssignmentTarget::Name(name) => name,
            _ => {
                return Err(LangError::Runtime(
                    "code generation backend does not yet support indexed assignments".into(),
                ))
            }
        };
        let var = self.lookup_var(target)?;
        let var_ty = self.variables[var.0].ty;
        let value = self.lower_assignment_value(var, var_ty, &assignment.kind)?;
        Ok(MirStatement::Assign { var, value })
    }

    fn lower_assignment_value(
        &mut self,
        var: VarId,
        var_ty: MirType,
        kind: &AssignmentKind,
    ) -> LangResult<MirExpr> {
        match kind {
            AssignmentKind::Simple(expr) => {
                let lowered = self.lower_expr(expr)?;
                var_ty.expect(lowered.ty, "assignment")?;
                Ok(lowered)
            }
            AssignmentKind::Compound { op, expr } => {
                if var_ty != MirType::Int {
                    return Err(LangError::Type(
                        "compound assignment currently supported only for integers".into(),
                    ));
                }
                let rhs = self.lower_expr(expr)?;
                var_ty.expect(rhs.ty, "compound assignment operand")?;
                let lhs = MirExpr::variable(var, var_ty);
                let mir_op = self.lower_binary_op(op)?;
                let ty = self.result_type(&mir_op, var_ty)?;
                Ok(MirExpr::binary(mir_op, lhs, rhs, ty))
            }
            AssignmentKind::Increment(op) => {
                if var_ty != MirType::Int {
                    return Err(LangError::Type(
                        "increment and decrement are only defined for integers".into(),
                    ));
                }
                let delta = match op {
                    IncrementOp::Increment => 1,
                    IncrementOp::Decrement => -1,
                };
                let lhs = MirExpr::variable(var, MirType::Int);
                let rhs = MirExpr::integer(delta);
                Ok(MirExpr::binary(MirBinaryOp::Add, lhs, rhs, MirType::Int))
            }
        }
    }

    fn lower_if(&mut self, if_stmt: &IfStatement) -> LangResult<MirStatement> {
        let cond = self.lower_expr(&if_stmt.condition)?;
        cond.ty.expect(MirType::Bool, "if condition")?;
        let then_body = self.lower_block(&if_stmt.then_branch)?;
        let else_body = match &if_stmt.else_branch {
            Some(branch) => self.lower_else_branch(branch.as_ref())?,
            None => Vec::new(),
        };
        Ok(MirStatement::If {
            cond,
            then_body,
            else_body,
        })
    }

    fn lower_else_branch(&mut self, branch: &ElseBranch) -> LangResult<Vec<MirStatement>> {
        match branch {
            ElseBranch::If(nested) => Ok(vec![self.lower_if(nested)?]),
            ElseBranch::Block(statements) => self.lower_block(statements),
        }
    }

    fn lower_while(&mut self, while_stmt: &WhileStatement) -> LangResult<MirStatement> {
        let cond = self.lower_expr(&while_stmt.condition)?;
        cond.ty.expect(MirType::Bool, "while condition")?;
        self.loop_depth += 1;
        let body = self.lower_block(&while_stmt.body)?;
        self.loop_depth -= 1;
        Ok(MirStatement::While { cond, body })
    }

    fn lower_expr(&mut self, expr: &Expr) -> LangResult<MirExpr> {
        match expr {
            Expr::Literal(lit) => self.lower_literal(lit),
            Expr::Variable(name) => {
                let var = self.lookup_var(name)?;
                let ty = self.variables[var.0].ty;
                Ok(MirExpr::variable(var, ty))
            }
            Expr::Unary { op, expr } => {
                let inner = self.lower_expr(expr)?;
                match op {
                    UnaryOp::Negate => {
                        inner.ty.expect(MirType::Int, "unary minus operand")?;
                        Ok(MirExpr::unary(MirType::Int, MirUnaryOp::Negate, inner))
                    }
                    UnaryOp::Not => {
                        inner.ty.expect(MirType::Bool, "logical not operand")?;
                        Ok(MirExpr::unary(MirType::Bool, MirUnaryOp::Not, inner))
                    }
                }
            }
            Expr::Binary { left, op, right } => {
                let left_expr = self.lower_expr(left)?;
                let right_expr = self.lower_expr(right)?;
                let mir_op = self.lower_binary_op(op)?;
                let result_ty = self.resolve_binary_type(&mir_op, &left_expr, &right_expr)?;
                Ok(MirExpr::binary(mir_op, left_expr, right_expr, result_ty))
            }
            Expr::Index { .. } => Err(LangError::Runtime(
                "code generation backend does not yet support array indexing".into(),
            )),
        }
    }

    fn lower_literal(&self, literal: &Literal) -> LangResult<MirExpr> {
        match literal {
            Literal::Integer(value) => Ok(MirExpr::integer(*value)),
            Literal::Bool(value) => Ok(MirExpr::boolean(*value)),
            Literal::Float(_) => Err(LangError::Runtime(
                "native code generation does not yet support floating point literals".into(),
            )),
            Literal::Str(_) => Err(LangError::Runtime(
                "native code generation does not yet support string literals".into(),
            )),
            Literal::Array(_) => Err(LangError::Runtime(
                "native code generation does not yet support array literals".into(),
            )),
        }
    }

    fn lower_binary_op(&self, op: &BinaryOp) -> LangResult<MirBinaryOp> {
        Ok(match op {
            BinaryOp::Add => MirBinaryOp::Add,
            BinaryOp::Subtract => MirBinaryOp::Subtract,
            BinaryOp::Multiply => MirBinaryOp::Multiply,
            BinaryOp::Divide => MirBinaryOp::Divide,
            BinaryOp::Modulo => MirBinaryOp::Modulo,
            BinaryOp::Equal => MirBinaryOp::Equal,
            BinaryOp::NotEqual => MirBinaryOp::NotEqual,
            BinaryOp::Less => MirBinaryOp::Less,
            BinaryOp::LessEqual => MirBinaryOp::LessEqual,
            BinaryOp::Greater => MirBinaryOp::Greater,
            BinaryOp::GreaterEqual => MirBinaryOp::GreaterEqual,
            BinaryOp::And => MirBinaryOp::And,
            BinaryOp::Or => MirBinaryOp::Or,
        })
    }

    fn resolve_binary_type(
        &self,
        op: &MirBinaryOp,
        left: &MirExpr,
        right: &MirExpr,
    ) -> LangResult<MirType> {
        match op {
            MirBinaryOp::Add
            | MirBinaryOp::Subtract
            | MirBinaryOp::Multiply
            | MirBinaryOp::Divide
            | MirBinaryOp::Modulo => {
                left.ty
                    .expect(MirType::Int, "binary operation left operand")?;
                right
                    .ty
                    .expect(MirType::Int, "binary operation right operand")?;
                Ok(MirType::Int)
            }
            MirBinaryOp::Equal | MirBinaryOp::NotEqual => {
                if left.ty != right.ty {
                    return Err(LangError::Type(
                        "equality comparison requires matching operand types".into(),
                    ));
                }
                match left.ty {
                    MirType::Int | MirType::Bool => Ok(MirType::Bool),
                }
            }
            MirBinaryOp::Less
            | MirBinaryOp::LessEqual
            | MirBinaryOp::Greater
            | MirBinaryOp::GreaterEqual => {
                left.ty.expect(MirType::Int, "comparison left operand")?;
                right.ty.expect(MirType::Int, "comparison right operand")?;
                Ok(MirType::Bool)
            }
            MirBinaryOp::And | MirBinaryOp::Or => {
                left.ty.expect(MirType::Bool, "logical operand")?;
                right.ty.expect(MirType::Bool, "logical operand")?;
                Ok(MirType::Bool)
            }
        }
    }

    fn result_type(&self, op: &MirBinaryOp, operand_ty: MirType) -> LangResult<MirType> {
        match op {
            MirBinaryOp::Add
            | MirBinaryOp::Subtract
            | MirBinaryOp::Multiply
            | MirBinaryOp::Divide
            | MirBinaryOp::Modulo => Ok(MirType::Int),
            MirBinaryOp::Equal
            | MirBinaryOp::NotEqual
            | MirBinaryOp::Less
            | MirBinaryOp::LessEqual
            | MirBinaryOp::Greater
            | MirBinaryOp::GreaterEqual
            | MirBinaryOp::And
            | MirBinaryOp::Or => {
                operand_ty.expect(MirType::Int, "compound assignment operand")?;
                Ok(MirType::Bool)
            }
        }
    }

    fn resolve_type(&self, annotation: Option<&TypeAnnotation>) -> LangResult<MirType> {
        let annotation = annotation.ok_or_else(|| {
            LangError::Runtime("explicit type annotation required for native compilation".into())
        })?;
        if !annotation.generics.is_empty() {
            return Err(LangError::Runtime(
                "generic type annotations are not yet supported by native backend".into(),
            ));
        }
        match annotation.name.to_ascii_lowercase().as_str() {
            "int" => Ok(MirType::Int),
            "bool" => Ok(MirType::Bool),
            other => Err(LangError::Runtime(format!(
                "native backend does not support type `{other}`"
            ))),
        }
    }

    fn push_variable(&mut self, name: String, ty: MirType, mutable: bool) -> VarId {
        let id = VarId(self.variables.len());
        self.variables.push(MirVariable {
            name: name.clone(),
            ty,
            mutable,
        });
        self.names.insert(name, id);
        id
    }

    fn lookup_var(&self, name: &str) -> LangResult<VarId> {
        self.names.get(name).copied().ok_or_else(|| {
            LangError::Runtime(format!("variable `{name}` referenced before declaration"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var_decl(name: &str, ty: &str, mutable: bool, value: Option<Expr>) -> Statement {
        Statement::Let(VarDeclaration::new(
            name.to_string(),
            Some(TypeAnnotation::new(ty.to_string())),
            mutable,
            value,
        ))
    }

    #[test]
    fn lowers_simple_program() {
        let statements = vec![
            var_decl(
                "counter",
                "int",
                true,
                Some(Expr::Literal(Literal::Integer(2))),
            ),
            Statement::Assignment(Assignment::new(
                AssignmentTarget::Name("counter".into()),
                AssignmentKind::Simple(Expr::Binary {
                    left: Box::new(Expr::Variable("counter".into())),
                    op: BinaryOp::Add,
                    right: Box::new(Expr::Literal(Literal::Integer(3))),
                }),
            )),
            Statement::Echo(Expr::Variable("counter".into())),
        ];

        let program = lower_to_mir(&statements).expect("lowering succeeds");
        assert_eq!(program.variables.len(), 1);
        match &program.body[0] {
            MirStatement::Let { .. } => {}
            other => panic!("unexpected statement: {other:?}"),
        }
    }
}
