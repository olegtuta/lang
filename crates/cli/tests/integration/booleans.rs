use super::run_script;

#[test]
fn evaluates_boolean_logic() {
    let output = run_script(
        "let flag: bool = true\necho flag && false",
    );
    assert_eq!(output, vec!["false".to_string()]);
}

#[test]
fn compares_numeric_values() {
    let output = run_script(
        "let lhs: int = 4\nlet rhs: int = 7\necho lhs < rhs && rhs == 7",
    );
    assert_eq!(output, vec!["true".to_string()]);
}
