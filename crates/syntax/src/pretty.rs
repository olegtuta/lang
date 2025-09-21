use crate::ast::{Assignment, BinaryOp, Expr, Literal, Statement, UnaryOp, VarDeclaration};

pub fn format_statement(statement: &Statement) -> String {
    match statement {
        Statement::VarDeclaration(decl) => format_var_declaration(decl),
        Statement::Assignment(assign) => format_assignment(assign),
        Statement::Echo(expr) => format!("echo {};", format_expr(expr)),
    }
}

fn format_var_declaration(decl: &VarDeclaration) -> String {
    let mut output = format!("{} {}", decl.ty.name, decl.name);
    if decl.mutable {
        output.push_str(" :=");
        if let Some(value) = &decl.value {
            output.push_str(&format!(" {}", format_expr(value)));
        }
    } else if let Some(value) = &decl.value {
        output.push_str(&format!(" = {}", format_expr(value)));
    }
    output.push(';');
    output
}

fn format_assignment(assign: &Assignment) -> String {
    format!("{} = {};", assign.name, format_expr(&assign.value))
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
    use crate::ast::{Statement, TypeAnnotation, VarDeclaration};

    #[test]
    fn formats_variable_declaration() {
        let decl = VarDeclaration::new(
            "count".to_string(),
            TypeAnnotation::new("int".to_string()),
            true,
            Some(Expr::Literal(Literal::Integer(1))),
        );
        let statement = Statement::VarDeclaration(decl);
        assert_eq!(format_statement(&statement), "int count := 1;");
    }
}
