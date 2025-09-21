use thiserror::Error;

pub type LangResult<T> = Result<T, LangError>;

#[derive(Debug, Error)]
pub enum LangError {
    #[error("Unknown type `{0}`")]
    UnknownType(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Type error: {0}")]
    Type(String),
    #[error("Runtime error: {0}")]
    Runtime(String),
}

impl LangError {
    pub fn parse(message: impl Into<String>) -> Self {
        LangError::Parse(message.into())
    }

    pub fn unknown_type(name: impl Into<String>) -> Self {
        LangError::UnknownType(name.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_format_errors() {
        let err = LangError::unknown_type("foo");
        assert_eq!(format!("{}", err), "Unknown type `foo`");
    }
}
