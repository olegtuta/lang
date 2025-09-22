//! Code generation backends live here.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use lang_syntax::{ast::Statement, format_statement};

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

    let program_source = statements
        .iter()
        .map(format_statement)
        .collect::<Vec<_>>()
        .join("\n");
    let literal = string_literal(&program_source);
    let main_source = generated_main(&literal);

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

fn generated_main(program_literal: &str) -> String {
    format!(
        "use lang_core::{{Interpreter, LangError}};\nuse lang_syntax::parse_program;\nuse lang_std::echo;\n\nfn main() {{\n    if let Err(err) = run_program() {{\n        eprintln!(\"{{}}\", err);\n        std::process::exit(1);\n    }}\n}}\n\nfn run_program() -> Result<(), LangError> {{\n    let source: &str = {program_literal};\n    let statements = parse_program(source).map_err(|err| LangError::parse(err.to_string()))?;\n    let mut interpreter = Interpreter::new();\n    for statement in statements {{\n        let values = interpreter.execute(statement)?;\n        for value in values {{\n            println!(\"{{}}\", echo(&value));\n        }}\n    }}\n    Ok(())\n}}\n"
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
