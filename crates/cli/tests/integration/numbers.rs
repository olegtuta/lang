use super::run_script;

#[test]
fn echoes_integer_arithmetic() {
    let output = run_script(
        "let counter: int = 10\necho counter / 2 + 3",
    );
    assert_eq!(output, vec!["8".to_string()]);
}

#[test]
fn reassigns_mutable_binding() {
    let output = run_script(
        "let total: int = 1\ntotal = total + 4\necho total",
    );
    assert_eq!(output, vec!["5".to_string()]);
}

#[test]
fn handles_float_operations() {
    let output = run_script(
        "let weight: float = 4.5\necho weight / 2.0",
    );
    assert_eq!(output, vec!["2.25".to_string()]);
}
