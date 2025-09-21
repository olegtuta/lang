use std::io::{self, Write};

use lang_core::{LangError, LangResult, TypeRegistry};
use lang_runtime::Scope;
use lang_syntax::parse_variable_declaration;

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
    let mut scope = Scope::new();

    println!("Async Lang REPL. Enter variable declarations or type :quit to exit.");

    while let Some(input) = read_multiline_input("lang> ")? {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == ":quit" || trimmed == ":exit" {
            break;
        }

        match parse_variable_declaration(&input, &registry) {
            Ok(declaration) => {
                let result = if let Some(value) = declaration.value.clone() {
                    scope.declare_with_value(&declaration.name, declaration.ty.clone(), value)
                } else {
                    scope.declare(&declaration.name, declaration.ty.clone())
                };

                match result {
                    Ok(()) => {
                        println!(
                            "Declared {} `{}`{}",
                            declaration.ty,
                            declaration.name,
                            match scope
                                .get(&declaration.name)
                                .and_then(|binding| binding.value())
                            {
                                Some(value) => format!(" with initial value {value}"),
                                None => String::new(),
                            }
                        );
                    }
                    Err(err) => {
                        eprintln!("error: {err}");
                    }
                }
            }
            Err(err) => {
                eprintln!("parse error: {err}");
            }
        }
    }

    println!("Goodbye!");

    Ok(())
}
