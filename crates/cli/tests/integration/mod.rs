use assert_cmd::prelude::*;
use assert_cmd::Command;

mod booleans;
mod errors;
mod numbers;
mod strings;

/// Run a REPL session with the provided script and collect observable outputs.
pub(crate) fn run_script(script: &str) -> Vec<String> {
    let output = invoke_repl(script);
    output
        .split("lang>")
        .filter_map(|segment| {
            let trimmed = segment.trim();
            if trimmed.is_empty() {
                return None;
            }
            if trimmed.chars().all(|ch| ch == '.' || ch.is_whitespace()) {
                return None;
            }
            Some(trimmed.to_string())
        })
        .collect()
}

fn invoke_repl(script: &str) -> String {
    let mut command = Command::cargo_bin("lang-cli").expect("lang-cli binary is built");
    command.arg("run");

    let mut input = script.trim().to_string();
    if !input.ends_with('\n') {
        input.push('\n');
    }
    input.push_str(":quit\n");

    let assert = command
        .write_stdin(input)
        .assert()
        .success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}
