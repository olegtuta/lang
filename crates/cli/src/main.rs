use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use lang_core::{compile_to_executable, BuildProfile, Interpreter, LangError, LangResult};
use lang_std::echo;
use lang_syntax::{format_statement, parse_program, parse_statement};

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
    /// Build a program from a source directory into a binary artifact path
    Build {
        source: PathBuf,
        output: PathBuf,
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
        Commands::Build {
            source,
            output,
            release,
        } => build_program(&source, &output, release),
        Commands::Fmt { input } => format_file(&input),
        Commands::Test { input } => test_program(&input),
    }
}

fn run_repl() -> LangResult<()> {
    let mut interpreter = Interpreter::new();
    let mut buffer = String::new();
    let mut line = String::new();
    loop {
        if buffer.is_empty() {
            print!("lang> ");
        } else {
            print!("....> ");
        }
        io::stdout()
            .flush()
            .map_err(|err| LangError::Runtime(format!("failed to flush stdout: {err}")))?;

        line.clear();
        let bytes_read = io::stdin()
            .read_line(&mut line)
            .map_err(|err| LangError::Runtime(format!("failed to read line: {err}")))?;

        if bytes_read == 0 {
            break;
        }

        let raw = line.trim_end_matches(['\r', '\n']);
        if buffer.is_empty() {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == ":quit" || trimmed == ":exit" {
                break;
            }
        }

        buffer.push_str(raw);
        buffer.push('\n');

        if !statement_complete(&buffer) {
            continue;
        }

        let source = buffer.trim();
        if source.is_empty() {
            buffer.clear();
            continue;
        }

        match parse_statement(source) {
            Ok(statement) => match interpreter.execute(statement) {
                Ok(values) => {
                    for value in values {
                        println!("{}", echo(&value));
                    }
                }
                Err(err) => {
                    eprintln!("error: {err}");
                }
            },
            Err(err) => {
                eprintln!("parse error: {err}");
            }
        }
        buffer.clear();
    }

    Ok(())
}

fn check_file(path: &Path) -> LangResult<()> {
    let source = read_source(path)?;
    let statements = parse_program(&source).map_err(|err| LangError::parse(err.to_string()))?;
    let mut interpreter = Interpreter::new();
    for statement in statements {
        let _ = interpreter.execute(statement)?;
    }
    Ok(())
}

fn build_program(source: &Path, output: &Path, release: bool) -> LangResult<()> {
    let files = collect_source_files(source)?;
    if files.is_empty() {
        return Err(LangError::Runtime(format!(
            "no source files with .lang extension found in {}",
            source.display()
        )));
    }

    let mut interpreter = Interpreter::new();
    let mut program = Vec::new();
    for file in files {
        let contents = read_source(&file)?;
        let statements =
            parse_program(&contents).map_err(|err| LangError::parse(err.to_string()))?;
        for statement in statements {
            let _ = interpreter.execute(statement.clone())?;
            program.push(statement);
        }
    }

    let profile = if release {
        BuildProfile::Release
    } else {
        BuildProfile::Dev
    };

    compile_to_executable(&program, output, profile)?;

    println!(
        "built {} ({} mode) from {}",
        output.display(),
        match profile {
            BuildProfile::Dev => "dev",
            BuildProfile::Release => "release",
        },
        source.display()
    );
    Ok(())
}

fn format_file(path: &Path) -> LangResult<()> {
    let source = read_source(path)?;
    let statements = parse_program(&source).map_err(|err| LangError::parse(err.to_string()))?;
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

fn statement_complete(buffer: &str) -> bool {
    let mut paren = 0i32;
    let mut brace = 0i32;
    let mut bracket = 0i32;
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = buffer.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '\\' if in_single || in_double => {
                chars.next();
            }
            '(' if !in_single && !in_double => paren += 1,
            ')' if !in_single && !in_double => paren -= 1,
            '{' if !in_single && !in_double => brace += 1,
            '}' if !in_single && !in_double => brace -= 1,
            '[' if !in_single && !in_double => bracket += 1,
            ']' if !in_single && !in_double => bracket -= 1,
            _ => {}
        }
        if paren < 0 || brace < 0 || bracket < 0 {
            return true;
        }
    }
    !in_single
        && !in_double
        && paren == 0
        && brace == 0
        && bracket == 0
        && !buffer.trim().is_empty()
}

fn collect_source_files(dir: &Path) -> LangResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)
        .map_err(|err| LangError::Runtime(format!("failed to read {}: {err}", dir.display())))?
    {
        let entry = entry.map_err(|err| {
            LangError::Runtime(format!(
                "failed to access directory entry in {}: {err}",
                dir.display()
            ))
        })?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|err| LangError::Runtime(format!("failed to stat {}: {err}", path.display())))?
            .is_dir()
        {
            files.extend(collect_source_files(&path)?);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("lang"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    Ok(files)
}
