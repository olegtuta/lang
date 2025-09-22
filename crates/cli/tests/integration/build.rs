use std::fs;
use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::tempdir;

#[test]
fn builds_and_executes_program() {
    let temp = tempdir().expect("create temp workspace");
    let project_dir = temp.path().join("project");
    fs::create_dir_all(&project_dir).expect("create project directory");

    let source = project_dir.join("main.lang");
    fs::write(
        &source,
        "let counter: int = 2\necho counter + 3\n",
    )
    .expect("write source");

    let binary_name = if cfg!(windows) { "program.exe" } else { "program" };
    let output_path = temp.path().join(binary_name);

    let mut cmd = Command::cargo_bin("lang-cli").expect("lang-cli binary available");
    cmd.arg("build")
        .arg(&project_dir)
        .arg(&output_path);

    cmd.assert().success();

    assert!(output_path.exists(), "expected binary to be created");

    let execution = Command::new(&output_path)
        .output()
        .expect("run compiled binary");

    assert!(execution.status.success(), "binary exited unsuccessfully");
    let stdout = String::from_utf8_lossy(&execution.stdout);
    assert_eq!(stdout.trim(), "5");
}
