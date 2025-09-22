use crate::ast::{
    ArrayElement, Assignment, AssignmentKind, AssignmentTarget, BinaryOp, ElseBranch, Expr,
    IfStatement, IncrementOp, IndexTarget, Literal, Statement, TypeAnnotation, UnaryOp,
    VarDeclaration, WhileStatement,
};

pub fn format_statement(statement: &Statement) -> String {
    match statement {
        Statement::Let(decl) => format_let_declaration(decl),
        Statement::Assignment(assign) => format_assignment(assign),
        Statement::Echo(expr) => format!("echo {}", format_expr(expr)),
        Statement::If(if_stmt) => format_if_statement(if_stmt),
        Statement::While(while_stmt) => format_while_statement(while_stmt),
        Statement::Break => "break".to_string(),
        Statement::Continue => "continue".to_string(),
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
        output.push_str(&format_type_annotation(ty));
    }
    if let Some(value) = &decl.value {
        output.push_str(" = ");
        output.push_str(&format_expr(value));
    }
    output
}

fn format_assignment(assign: &Assignment) -> String {
    let target = match &assign.target {
        AssignmentTarget::Name(name) => name.clone(),
        AssignmentTarget::Indexed { name, indices } => {
            let mut output = name.clone();
            for index in indices {
                match index {
                    IndexTarget::Append => output.push_str("[]"),
                    IndexTarget::Index(expr) => {
                        output.push('[');
                        output.push_str(&format_expr(expr));
                        output.push(']');
                    }
                }
            }
            output
        }
    };

    match &assign.kind {
        AssignmentKind::Simple(expr) => format!("{} = {}", target, format_expr(expr)),
        AssignmentKind::Compound { op, expr } => {
            format!(
                "{} {}= {}",
                target,
                format_compound_op(*op),
                format_expr(expr)
            )
        }
        AssignmentKind::Increment(IncrementOp::Increment) => format!("{}++", target),
        AssignmentKind::Increment(IncrementOp::Decrement) => format!("{}--", target),
    }
}

fn format_if_statement(if_stmt: &IfStatement) -> String {
    let mut output = String::new();
    output.push_str("if (");
    output.push_str(&format_expr(&if_stmt.condition));
    output.push_str(") ");
    output.push_str(&format_block(&if_stmt.then_branch));
    if let Some(else_branch) = &if_stmt.else_branch {
        output.push(' ');
        match else_branch.as_ref() {
            ElseBranch::If(nested) => {
                output.push_str("else ");
                output.push_str(&format_if_statement(nested));
            }
            ElseBranch::Block(block) => {
                output.push_str("else ");
                output.push_str(&format_block(block));
            }
        }
    }
    output
}

fn format_while_statement(while_stmt: &WhileStatement) -> String {
    let mut output = String::new();
    output.push_str("while (");
    output.push_str(&format_expr(&while_stmt.condition));
    output.push_str(") ");
    output.push_str(&format_block(&while_stmt.body));
    output
}

fn format_block(statements: &[Statement]) -> String {
    if statements.is_empty() {
        return "{}".to_string();
    }
    let mut output = String::from("{\n");
    for statement in statements {
        output.push_str("  ");
        output.push_str(&format_statement(statement));
        output.push('\n');
    }
    output.push('}');
    output
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal(value) => format_literal(value),
        Expr::Variable(name) => name.clone(),
        Expr::Index { target, index } => {
            format!("{}[{}]", format_expr(target), format_expr(index))
        }
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

fn format_literal(value: &Literal) -> String {
    match value {
        Literal::Integer(v) => v.to_string(),
        Literal::Float(v) => v.to_string(),
        Literal::Bool(v) => v.to_string(),
        Literal::Str(v) => format!("\"{}\"", v.replace('"', "\\\"")),
        Literal::Array(elements) => format_array(elements),
    }
}

fn format_array(elements: &[ArrayElement]) -> String {
    let mut parts = Vec::new();
    for element in elements {
        match element {
            ArrayElement::Value(expr) => parts.push(format_expr(expr)),
            ArrayElement::KeyValue { key, value } => {
                parts.push(format!("{} => {}", format_expr(key), format_expr(value)))
            }
        }
    }
    format!("[{}]", parts.join(", "))
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

fn format_type_annotation(annotation: &TypeAnnotation) -> String {
    if annotation.generics.is_empty() {
        annotation.name.clone()
    } else {
        let generics: Vec<String> = annotation
            .generics
            .iter()
            .map(format_type_annotation)
            .collect();
        format!("{}<{}>", annotation.name, generics.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn formats_if_else() {
        let stmt = Statement::If(IfStatement::new(
            Expr::Literal(Literal::Bool(true)),
            vec![Statement::Echo(Expr::Literal(Literal::Integer(1)))],
            Some(Box::new(ElseBranch::Block(vec![Statement::Echo(
                Expr::Literal(Literal::Integer(2)),
            )]))),
        ));
        let formatted = format_statement(&stmt);
        assert!(formatted.contains("if"));
        assert!(formatted.contains("else"));
    }
}
