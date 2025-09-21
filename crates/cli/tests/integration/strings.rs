use super::run_script;

#[test]
fn concatenates_strings() {
    let output = run_script(
        "str greeting = \"hi\";\necho greeting + \" there\";",
    );
    assert_eq!(output, vec!["hi there".to_string()]);
}

#[test]
fn echoes_string_literal() {
    let output = run_script("echo \"plain text\";");
    assert_eq!(output, vec!["plain text".to_string()]);
}
