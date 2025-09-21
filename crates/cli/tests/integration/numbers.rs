use super::run_script;

#[test]
fn echoes_integer_arithmetic() {
    let output = run_script(
        "int counter = 10;\necho counter / 2 + 3;",
    );
    assert_eq!(output, vec!["8".to_string()]);
}

#[test]
fn handles_float_operations() {
    let output = run_script(
        "float weight = 4.5;\necho weight / 2.0;",
    );
    assert_eq!(output, vec!["2.25".to_string()]);
}
