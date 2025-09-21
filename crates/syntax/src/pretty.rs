use crate::ast::{
    Assignment, AssignmentKind, BinaryOp, Expr, IncrementOp, Literal, Statement, UnaryOp,
    VarDeclaration,
};

pub fn format_statement(statement: &Statement) -> String {
    match statement {
        Statement::Let(decl) => format_let_declaration(decl),
        Statement::Assignment(assign) => format_assignment(assign),
        Statement::Echo(expr) => format!("echo {}", format_expr(expr)),
    }
}

fn format_let_declaration(decl: &VarDeclaration) -> String {
    let mut output = String::from("let ");
    if !decl.mutable {
        output.push_str("fix ");
    }
    output.push_str(&decl.name);
    if let Some(ty) = &decl.ty {
        output.push_str(": ");
        output.push_str(&ty.name);
    }
    if let Some(value) = &decl.value {
        output.push_str(" = ");
        output.push_str(&format_expr(value));
    }
    output
}

fn format_assignment(assign: &Assignment) -> String {
    match &assign.kind {
        AssignmentKind::Simple(expr) => format!("{} = {}", assign.name, format_expr(expr)),
        AssignmentKind::Compound { op, expr } => {
            format!(
                "{} {}= {}",
                assign.name,
                format_compound_op(*op),
                format_expr(expr)
            )
        }
        AssignmentKind::Increment(IncrementOp::Increment) => format!("{}++", assign.name),
        AssignmentKind::Increment(IncrementOp::Decrement) => format!("{}--", assign.name),
    }
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal(value) => format_literal(value),
        Expr::Variable(name) => name.clone(),
        Expr::Unary { op, expr } => match op {
            UnaryOp::Negate => format!("-{}", wrap_unary(expr)),
            UnaryOp::Not => format!("!{}", wrap_unary(expr)),
        },
        Expr::Binary { left, op, right } => format!(
            "({} {} {})",
            format_expr(left),
            format_binary_op(*op),
            format_expr(right)
        ),
    }
}

fn wrap_unary(expr: &Expr) -> String {
    match expr {
        Expr::Literal(_) | Expr::Variable(_) => format_expr(expr),
        _ => format!("({})", format_expr(expr)),
    }
}

fn format_binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Modulo => "%",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

fn format_compound_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Modulo => "%",
        BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::Greater
        | BinaryOp::GreaterEqual
        | BinaryOp::And
        | BinaryOp::Or => unreachable!("invalid compound assignment operator"),
    }
}

fn format_literal(value: &Literal) -> String {
    match value {
        Literal::Integer(v) => v.to_string(),
        Literal::Float(v) => v.to_string(),
        Literal::Bool(v) => v.to_string(),
        Literal::Str(v) => format!("\"{}\"", v.replace('"', "\\\"")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{AssignmentKind, Statement, TypeAnnotation};

    #[test]
    fn formats_variable_declaration() {
        let decl = VarDeclaration::new(
            "count".to_string(),
            Some(TypeAnnotation::new("int".to_string())),
            true,
            Some(Expr::Literal(Literal::Integer(1))),
        );
        let statement = Statement::Let(decl);
        assert_eq!(format_statement(&statement), "let count: int = 1");
    }

    #[test]
    fn formats_increment_assignment() {
        let statement = Statement::Assignment(Assignment::new(
            "index".to_string(),
            AssignmentKind::Increment(IncrementOp::Increment),
        ));
        assert_eq!(format_statement(&statement), "index++");
    }
}
