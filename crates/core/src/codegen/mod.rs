//! Code generation backends live here.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use lang_syntax::ast::{
    ArrayElement, AssignmentKind, AssignmentTarget, BinaryOp, ElseBranch, Expr, IfStatement,
    IncrementOp, IndexTarget, Literal, Statement, TypeAnnotation, UnaryOp,
};
use lang_syntax::format_statement;

use crate::diagnostics::{LangError, LangResult};

const PACKAGE_NAME: &str = "lang_build_artifact";

/// Build configuration for artifact generation.
#[derive(Debug, Clone, Copy)]
pub enum BuildProfile {
    Dev,
    Release,
}

impl BuildProfile {
    fn cargo_flag(self) -> Option<&'static str> {
        match self {
            BuildProfile::Dev => None,
            BuildProfile::Release => Some("--release"),
        }
    }

    fn target_dir(self) -> &'static str {
        match self {
            BuildProfile::Dev => "debug",
            BuildProfile::Release => "release",
        }
    }
}

/// Compile the supplied statements into a standalone executable written to `output`.
pub fn compile_to_executable(
    statements: &[Statement],
    output: &Path,
    profile: BuildProfile,
) -> LangResult<()> {
    let build_dir = tempfile::Builder::new()
        .prefix("lang-build-")
        .tempdir()
        .map_err(|err| LangError::Runtime(format!("failed to create tempdir: {err}")))?;

    let workspace_root = workspace_root()?;
    let crate_dir = build_dir.path();
    fs::create_dir_all(crate_dir.join("src")).map_err(|err| {
        LangError::Runtime(format!("failed to create build source directory: {err}"))
    })?;

    let manifest = crate_manifest(&workspace_root);
    fs::write(crate_dir.join("Cargo.toml"), manifest)
        .map_err(|err| LangError::Runtime(format!("failed to write temporary manifest: {err}")))?;

    let formatted_program = statements
        .iter()
        .map(format_statement)
        .collect::<Vec<_>>()
        .join("\n");
    let literal = string_literal(&formatted_program);
    let main_source = generated_main(statements, &literal);

    let mut file = fs::File::create(crate_dir.join("src/main.rs"))
        .map_err(|err| LangError::Runtime(format!("failed to create generated main.rs: {err}")))?;
    file.write_all(main_source.as_bytes())
        .map_err(|err| LangError::Runtime(format!("failed to write generated main.rs: {err}")))?;

    let mut command = Command::new("cargo");
    command.arg("build").current_dir(crate_dir);
    if let Some(flag) = profile.cargo_flag() {
        command.arg(flag);
    }

    let status = command
        .status()
        .map_err(|err| LangError::Runtime(format!("failed to invoke cargo: {err}")))?;
    if !status.success() {
        return Err(LangError::Runtime("cargo build failed".into()));
    }

    let binary_name = if cfg!(windows) {
        format!("{PACKAGE_NAME}.exe")
    } else {
        PACKAGE_NAME.to_string()
    };
    let built_path = crate_dir
        .join("target")
        .join(profile.target_dir())
        .join(&binary_name);

    if !built_path.exists() {
        return Err(LangError::Runtime(format!(
            "expected compiled artifact at {}",
            built_path.display()
        )));
    }

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                LangError::Runtime(format!("failed to create {}: {err}", parent.display()))
            })?;
        }
    }

    fs::copy(&built_path, output).map_err(|err| {
        LangError::Runtime(format!(
            "failed to copy artifact from {} to {}: {err}",
            built_path.display(),
            output.display()
        ))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(output)
            .map_err(|err| LangError::Runtime(format!("failed to read permissions: {err}")))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(output, perms).map_err(|err| {
            LangError::Runtime(format!(
                "failed to set permissions on {}: {err}",
                output.display()
            ))
        })?;
    }

    Ok(())
}

fn crate_manifest(workspace_root: &Path) -> String {
    let core_path = workspace_root.join("crates/core");
    let std_path = workspace_root.join("crates/std");
    let syntax_path = workspace_root.join("crates/syntax");

    format!(
        "[package]\nname = \"{PACKAGE_NAME}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nlang-core = {{ path = \"{}\" }}\nlang-std = {{ path = \"{}\" }}\nlang-syntax = {{ path = \"{}\" }}\n",
        toml_path(&core_path),
        toml_path(&std_path),
        toml_path(&syntax_path)
    )
}

fn generated_main(statements: &[Statement], program_literal: &str) -> String {
    let ast_construction = statements
        .iter()
        .map(render_statement)
        .collect::<Vec<_>>()
        .join(",\n        ");

    format!(
        "use lang_core::{{Interpreter, LangError}};\nuse lang_std::echo;\nuse lang_syntax::ast::{{ArrayElement, Assignment, AssignmentKind, AssignmentTarget, BinaryOp, ElseBranch, Expr, IfStatement, IndexTarget, IncrementOp, Literal, Statement, TypeAnnotation, UnaryOp, VarDeclaration, WhileStatement}};\n\nfn main() {{\n    if let Err(err) = run_program() {{\n        eprintln!(\"{{}}\", err);\n        std::process::exit(1);\n    }}\n}}\n\nfn run_program() -> Result<(), LangError> {{\n    let program = build_program();\n    let mut interpreter = Interpreter::new();\n    for statement in program {{\n        let values = interpreter.execute(statement)?;\n        for value in values {{\n            println!(\"{{}}\", echo(&value));\n        }}\n    }}\n    Ok(())\n}}\n\nfn build_program() -> Vec<Statement> {{\n    // Original formatted source:\n    // {program_literal}\n    vec![\n        {ast_construction}\n    ]\n}}\n"
    )
}

fn string_literal(input: &str) -> String {
    let mut literal = String::with_capacity(input.len() + 2);
    literal.push('"');
    for ch in input.chars() {
        match ch {
            '\\' => literal.push_str("\\\\"),
            '"' => literal.push_str("\\\""),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            ch => literal.push(ch),
        }
    }
    literal.push('"');
    literal
}

fn render_statement(statement: &Statement) -> String {
    match statement {
        Statement::Let(var) => format!(
            "Statement::Let(VarDeclaration::new({name}, {ty}, {mutable}, {value}))",
            name = string_literal(&var.name),
            ty = render_option_type(&var.ty),
            mutable = var.mutable,
            value = render_option_expr(&var.value)
        ),
        Statement::Assignment(assign) => format!(
            "Statement::Assignment(Assignment::new({target}, {kind}))",
            target = render_assignment_target(&assign.target),
            kind = render_assignment_kind(&assign.kind)
        ),
        Statement::Echo(expr) => format!("Statement::Echo({})", render_expr(expr)),
        Statement::If(if_stmt) => format!(
            "Statement::If(IfStatement::new({condition}, vec![{then_branch}], {else_branch}))",
            condition = render_expr(&if_stmt.condition),
            then_branch = render_statement_list(&if_stmt.then_branch),
            else_branch = render_option_else_branch(if_stmt.else_branch.as_deref())
        ),
        Statement::While(while_stmt) => format!(
            "Statement::While(WhileStatement::new({condition}, vec![{body}]))",
            condition = render_expr(&while_stmt.condition),
            body = render_statement_list(&while_stmt.body)
        ),
        Statement::Break => "Statement::Break".into(),
        Statement::Continue => "Statement::Continue".into(),
    }
}

fn render_statement_list(statements: &[Statement]) -> String {
    statements
        .iter()
        .map(render_statement)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_option_type(ty: &Option<TypeAnnotation>) -> String {
    match ty {
        Some(annotation) => format!("Some({})", render_type_annotation(annotation)),
        None => "None".into(),
    }
}

fn render_type_annotation(ty: &TypeAnnotation) -> String {
    if ty.generics.is_empty() {
        format!("TypeAnnotation::new({})", string_literal(&ty.name))
    } else {
        let generics = ty
            .generics
            .iter()
            .map(render_type_annotation)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "TypeAnnotation::with_generics({}, vec![{}])",
            string_literal(&ty.name),
            generics
        )
    }
}

fn render_option_expr(expr: &Option<Expr>) -> String {
    match expr {
        Some(expr) => format!("Some({})", render_expr(expr)),
        None => "None".into(),
    }
}

fn render_assignment_target(target: &AssignmentTarget) -> String {
    match target {
        AssignmentTarget::Name(name) => {
            format!("AssignmentTarget::Name({})", string_literal(name))
        }
        AssignmentTarget::Indexed { name, indices } => format!(
            "AssignmentTarget::Indexed {{ name: {name}, indices: vec![{indices}] }}",
            name = string_literal(name),
            indices = indices
                .iter()
                .map(render_index_target)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_index_target(target: &IndexTarget) -> String {
    match target {
        IndexTarget::Index(expr) => {
            format!("IndexTarget::Index({})", render_expr(expr))
        }
        IndexTarget::Append => "IndexTarget::Append".into(),
    }
}

fn render_assignment_kind(kind: &AssignmentKind) -> String {
    match kind {
        AssignmentKind::Simple(expr) => format!("AssignmentKind::Simple({})", render_expr(expr)),
        AssignmentKind::Compound { op, expr } => format!(
            "AssignmentKind::Compound {{ op: {op}, expr: {expr} }}",
            op = render_binary_op(*op),
            expr = render_expr(expr)
        ),
        AssignmentKind::Increment(op) => {
            format!("AssignmentKind::Increment({})", render_increment_op(*op))
        }
    }
}

fn render_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal(lit) => format!("Expr::Literal({})", render_literal(lit)),
        Expr::Variable(name) => format!("Expr::Variable({})", string_literal(name)),
        Expr::Index { target, index } => format!(
            "Expr::Index {{ target: Box::new({target}), index: Box::new({index}) }}",
            target = render_expr(target),
            index = render_expr(index)
        ),
        Expr::Unary { op, expr } => format!(
            "Expr::Unary {{ op: {op}, expr: Box::new({expr}) }}",
            op = render_unary_op(*op),
            expr = render_expr(expr)
        ),
        Expr::Binary { left, op, right } => format!(
            "Expr::Binary {{ left: Box::new({left}), op: {op}, right: Box::new({right}) }}",
            left = render_expr(left),
            op = render_binary_op(*op),
            right = render_expr(right)
        ),
    }
}

fn render_literal(literal: &Literal) -> String {
    match literal {
        Literal::Integer(value) => format!("Literal::Integer({})", value),
        Literal::Float(value) => format!("Literal::Float({})", value),
        Literal::Bool(value) => format!("Literal::Bool({})", value),
        Literal::Str(value) => format!("Literal::Str({})", string_literal(value)),
        Literal::Array(elements) => format!(
            "Literal::Array(vec![{}])",
            elements
                .iter()
                .map(render_array_element)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_array_element(element: &ArrayElement) -> String {
    match element {
        ArrayElement::Value(expr) => {
            format!("ArrayElement::Value({})", render_expr(expr))
        }
        ArrayElement::KeyValue { key, value } => format!(
            "ArrayElement::KeyValue {{ key: {key}, value: {value} }}",
            key = render_expr(key),
            value = render_expr(value)
        ),
    }
}

fn render_option_else_branch(branch: Option<&ElseBranch>) -> String {
    match branch {
        Some(ElseBranch::If(inner)) => {
            format!("Some(Box::new(ElseBranch::If({})))", render_if(inner))
        }
        Some(ElseBranch::Block(statements)) => format!(
            "Some(Box::new(ElseBranch::Block(vec![{}])))",
            render_statement_list(statements)
        ),
        None => "None".into(),
    }
}

fn render_if(if_stmt: &IfStatement) -> String {
    format!(
        "IfStatement::new({condition}, vec![{then_branch}], {else_branch})",
        condition = render_expr(&if_stmt.condition),
        then_branch = render_statement_list(&if_stmt.then_branch),
        else_branch = render_option_else_branch(if_stmt.else_branch.as_deref())
    )
}

fn render_unary_op(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Negate => "UnaryOp::Negate",
        UnaryOp::Not => "UnaryOp::Not",
    }
}

fn render_binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "BinaryOp::Add",
        BinaryOp::Subtract => "BinaryOp::Subtract",
        BinaryOp::Multiply => "BinaryOp::Multiply",
        BinaryOp::Divide => "BinaryOp::Divide",
        BinaryOp::Modulo => "BinaryOp::Modulo",
        BinaryOp::Equal => "BinaryOp::Equal",
        BinaryOp::NotEqual => "BinaryOp::NotEqual",
        BinaryOp::Less => "BinaryOp::Less",
        BinaryOp::LessEqual => "BinaryOp::LessEqual",
        BinaryOp::Greater => "BinaryOp::Greater",
        BinaryOp::GreaterEqual => "BinaryOp::GreaterEqual",
        BinaryOp::And => "BinaryOp::And",
        BinaryOp::Or => "BinaryOp::Or",
    }
}

fn render_increment_op(op: IncrementOp) -> &'static str {
    match op {
        IncrementOp::Increment => "IncrementOp::Increment",
        IncrementOp::Decrement => "IncrementOp::Decrement",
    }
}

fn toml_path(path: &Path) -> String {
    path.to_str()
        .map(|raw| raw.replace('\\', "\\\\"))
        .unwrap_or_else(|| path.display().to_string())
}

fn workspace_root() -> LangResult<PathBuf> {
    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    core_dir
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .ok_or_else(|| LangError::Runtime("failed to determine workspace root".into()))
}
