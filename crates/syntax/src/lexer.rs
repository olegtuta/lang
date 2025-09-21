use lang_core::{LangError, LangResult};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    LBracket,
    RBracket,
    Comma,
    Dollar,
    Equals,
    Semicolon,
    Identifier(String),
    IntegerLiteral(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub position: usize,
}

pub fn lex(input: &str) -> LangResult<Vec<Token>> {
    let mut chars = input.char_indices().peekable();
    let mut tokens = Vec::new();

    while let Some((idx, ch)) = chars.peek().cloned() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        let token = match ch {
            '[' => {
                chars.next();
                Token {
                    kind: TokenKind::LBracket,
                    position: idx,
                }
            }
            ']' => {
                chars.next();
                Token {
                    kind: TokenKind::RBracket,
                    position: idx,
                }
            }
            ',' => {
                chars.next();
                Token {
                    kind: TokenKind::Comma,
                    position: idx,
                }
            }
            '$' => {
                chars.next();
                Token {
                    kind: TokenKind::Dollar,
                    position: idx,
                }
            }
            '=' => {
                chars.next();
                Token {
                    kind: TokenKind::Equals,
                    position: idx,
                }
            }
            ';' => {
                chars.next();
                Token {
                    kind: TokenKind::Semicolon,
                    position: idx,
                }
            }
            ch if ch.is_ascii_digit() => {
                while let Some(&(_, next_ch)) = chars.peek() {
                    if next_ch.is_ascii_digit() {
                        chars.next();
                    } else {
                        break;
                    }
                }
                let slice_end = chars
                    .peek()
                    .map(|&(next_idx, _)| next_idx)
                    .unwrap_or_else(|| input.len());
                let literal = &input[idx..slice_end];
                let value = literal.parse::<i64>().map_err(|err| {
                    LangError::parse(format!(
                        "failed to parse integer literal `{literal}` at position {idx}: {err}"
                    ))
                })?;
                Token {
                    kind: TokenKind::IntegerLiteral(value),
                    position: idx,
                }
            }
            ch if is_identifier_start(ch) => {
                while let Some(&(_, next_ch)) = chars.peek() {
                    if is_identifier_part(next_ch) {
                        chars.next();
                    } else {
                        break;
                    }
                }
                let slice_end = chars
                    .peek()
                    .map(|&(next_idx, _)| next_idx)
                    .unwrap_or_else(|| input.len());
                let ident = input[idx..slice_end].to_string();
                Token {
                    kind: TokenKind::Identifier(ident),
                    position: idx,
                }
            }
            _ => {
                return Err(LangError::parse(format!(
                    "unexpected character `{}` at position {}",
                    ch, idx
                )));
            }
        };

        tokens.push(token);
    }

    Ok(tokens)
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_variable_declaration() {
        let tokens = lex("[int, mute] $value = 10;").unwrap();
        let kinds: Vec<_> = tokens.iter().map(|t| &t.kind).collect();
        assert!(matches!(kinds[0], TokenKind::LBracket));
        assert!(matches!(kinds[1], TokenKind::Identifier(_)));
        assert!(matches!(kinds[2], TokenKind::Comma));
        assert!(matches!(kinds[3], TokenKind::Identifier(_)));
        assert!(matches!(kinds[4], TokenKind::RBracket));
        assert!(matches!(kinds[5], TokenKind::Dollar));
        assert!(matches!(kinds[6], TokenKind::Identifier(_)));
        assert!(matches!(kinds[7], TokenKind::Equals));
        assert!(matches!(kinds[8], TokenKind::IntegerLiteral(10)));
        assert!(matches!(kinds[9], TokenKind::Semicolon));
    }
}
