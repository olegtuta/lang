use std::io::{self, Write};

use lang_core::{LangError, LangResult, TypeRegistry};
use lang_runtime::Interpreter;
use lang_syntax::parse_statement;

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

fn main() -> LangResult<()> {
    let registry = TypeRegistry::new();
    let mut interpreter = Interpreter::new();

    while let Some(input) = read_multiline_input("lang> ")? {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == ":quit" || trimmed == ":exit" {
            break;
        }

        match parse_statement(&input, &registry) {
            Ok(statement) => match interpreter.execute(statement) {
                Ok(Some(value)) => {
                    println!("{value}");
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
