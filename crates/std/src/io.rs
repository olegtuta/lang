use lang_core::Value;

pub fn echo(value: &Value) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_formats_value() {
        let value = Value::from(42);
        assert_eq!(echo(&value), "42");
    }
}
