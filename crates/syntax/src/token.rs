#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    LParen,
    RParen,
    Equals,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    DoubleAmpersand,
    DoublePipe,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    EqualEqual,
    BangEqual,
    Identifier(String),
    IntegerLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    BoolLiteral(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub position: usize,
}
