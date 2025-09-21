use thiserror::Error;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct SyntaxError {
    message: String,
}

impl SyntaxError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub type SyntaxResult<T> = Result<T, SyntaxError>;
