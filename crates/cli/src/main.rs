use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use lang_core::{Interpreter, LangError, LangResult};
use lang_std::echo;
use lang_syntax::{format_statement, parse_statement, Statement};

#[derive(Parser)]
#[command(
    name = "lang",
    version,
    about = "Command line tools for the lang project"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start an interactive REPL session
    Run,
    /// Type-check and execute declarations from a source file without producing build artifacts
    Check { input: PathBuf },
    /// Build a program (currently validates only)
    Build {
        input: PathBuf,
        #[arg(long)]
        release: bool,
    },
    /// Format a source file in place
    Fmt { input: PathBuf },
    /// Run test sources (currently validates only)
    Test { input: PathBuf },
}

fn main() -> LangResult<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run => run_repl(),
        Commands::Check { input } => check_file(&input),
        Commands::Build { input, release } => build_program(&input, release),
        Commands::Fmt { input } => format_file(&input),
        Commands::Test { input } => test_program(&input),
    }
}

fn run_repl() -> LangResult<()> {
    let mut interpreter = Interpreter::new();

    while let Some(input) = read_multiline_input("lang> ")? {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == ":quit" || trimmed == ":exit" {
            break;
        }

        match parse_statement(&input) {
            Ok(statement) => match interpreter.execute(statement) {
                Ok(Some(value)) => {
                    println!("{}", echo(&value));
                }
                Ok(None) => {}
                Err(err) => {
                    eprintln!("error: {err}");
                }
            },
            Err(err) => {
                eprintln!("parse error: {err}");
            }
        }
    }

    Ok(())
}

fn read_multiline_input(prompt: &str) -> LangResult<Option<String>> {
    let mut buffer = String::new();

    loop {
        let current_prompt = if buffer.is_empty() { prompt } else { "... " };
        print!("{}", current_prompt);
        io::stdout()
            .flush()
            .map_err(|err| LangError::Runtime(format!("failed to flush stdout: {err}")))?;

        let mut line = String::new();
        let bytes_read = io::stdin()
            .read_line(&mut line)
            .map_err(|err| LangError::Runtime(format!("failed to read line: {err}")))?;
        if bytes_read == 0 {
            if buffer.trim().is_empty() {
                return Ok(None);
            }
            break;
        }

        buffer.push_str(&line);

        let trimmed = buffer.trim_end();
        if trimmed == ":quit" || trimmed == ":exit" {
            break;
        }

        if trimmed.ends_with(';') {
            break;
        }
    }

    Ok(Some(buffer))
}

fn check_file(path: &Path) -> LangResult<()> {
    let source = read_source(path)?;
    let statements = parse_program(&source)?;
    let mut interpreter = Interpreter::new();
    for statement in statements {
        interpreter.execute(statement)?;
    }
    Ok(())
}

fn build_program(path: &Path, release: bool) -> LangResult<()> {
    check_file(path)?;
    let profile = if release { "release" } else { "dev" };
    println!("build succeeded ({profile} mode)");
    Ok(())
}

fn format_file(path: &Path) -> LangResult<()> {
    let source = read_source(path)?;
    let statements = parse_program(&source)?;
    let formatted: Vec<String> = statements.iter().map(format_statement).collect();
    fs::write(path, formatted.join("\n"))
        .map_err(|err| LangError::Runtime(format!("failed to write file: {err}")))?;
    println!("formatted {}", path.display());
    Ok(())
}

fn test_program(path: &Path) -> LangResult<()> {
    check_file(path)?;
    println!("test suite passed (no runtime harness yet)");
    Ok(())
}

fn read_source(path: &Path) -> LangResult<String> {
    fs::read_to_string(path)
        .map_err(|err| LangError::Runtime(format!("failed to read {}: {err}", path.display())))
}

fn parse_program(source: &str) -> LangResult<Vec<Statement>> {
    let mut statements = Vec::new();
    for chunk in split_statements(source)? {
        let parsed = parse_statement(&chunk).map_err(|err| LangError::parse(err.to_string()))?;
        statements.push(parsed);
    }
    Ok(statements)
}

fn split_statements(source: &str) -> LangResult<Vec<String>> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaping = false;

    for ch in source.chars() {
        current.push(ch);
        if in_string {
            if escaping {
                escaping = false;
                continue;
            }
            if ch == '\\' {
                escaping = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            continue;
        }
        if ch == ';' {
            if !current.trim().is_empty() {
                statements.push(current.clone());
            }
            current.clear();
        }
    }

    if in_string {
        return Err(LangError::parse("unterminated string literal"));
    }

    if !current.trim().is_empty() {
        return Err(LangError::parse("expected ';' to terminate statement"));
    }

    Ok(statements)
}
