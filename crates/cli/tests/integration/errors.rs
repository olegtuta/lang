use super::run_script;

#[test]
fn prevents_reassigning_immutable_variables() {
    let output = run_script("int value = 1;\nvalue = 2;");
    assert_eq!(
        output,
        vec!["error: Type error: attempt to reassign immutable binding".to_string()],
    );
}

#[test]
fn surfaces_type_mismatches() {
    let output = run_script("int value = 1;\necho value == \"1\";");
    assert_eq!(
        output,
        vec![
            "error: Type error: equality comparison requires matching operand types"
                .to_string(),
        ],
    );
}

#[test]
fn reports_missing_bindings() {
    let output = run_script("echo missing;");
    assert_eq!(
        output,
        vec!["error: Runtime error: variable `missing` is not defined".to_string()],
    );
}
