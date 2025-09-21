use crate::error::{SyntaxError, SyntaxResult};
use crate::token::{Token, TokenKind};

pub fn lex(input: &str) -> SyntaxResult<Vec<Token>> {
    let mut chars = input.char_indices().peekable();
    let mut tokens = Vec::new();

    while let Some((idx, ch)) = chars.peek().cloned() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        let token = match ch {
            '(' => {
                chars.next();
                Token {
                    kind: TokenKind::LParen,
                    position: idx,
                }
            }
            ')' => {
                chars.next();
                Token {
                    kind: TokenKind::RParen,
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
            '+' => {
                chars.next();
                Token {
                    kind: TokenKind::Plus,
                    position: idx,
                }
            }
            '-' => {
                chars.next();
                Token {
                    kind: TokenKind::Minus,
                    position: idx,
                }
            }
            '*' => {
                chars.next();
                Token {
                    kind: TokenKind::Star,
                    position: idx,
                }
            }
            '/' => {
                chars.next();
                Token {
                    kind: TokenKind::Slash,
                    position: idx,
                }
            }
            '%' => {
                chars.next();
                Token {
                    kind: TokenKind::Percent,
                    position: idx,
                }
            }
            '=' => {
                chars.next();
                if matches!(chars.peek(), Some(&(_, '='))) {
                    chars.next();
                    Token {
                        kind: TokenKind::EqualEqual,
                        position: idx,
                    }
                } else {
                    Token {
                        kind: TokenKind::Equals,
                        position: idx,
                    }
                }
            }
            '!' => {
                chars.next();
                if matches!(chars.peek(), Some(&(_, '='))) {
                    chars.next();
                    Token {
                        kind: TokenKind::BangEqual,
                        position: idx,
                    }
                } else {
                    Token {
                        kind: TokenKind::Bang,
                        position: idx,
                    }
                }
            }
            '&' => {
                chars.next();
                if matches!(chars.peek(), Some(&(_, '&'))) {
                    chars.next();
                    Token {
                        kind: TokenKind::DoubleAmpersand,
                        position: idx,
                    }
                } else {
                    return Err(SyntaxError::new(format!(
                        "unexpected character `&` at position {idx}; did you mean `&&`?"
                    )));
                }
            }
            '|' => {
                chars.next();
                if matches!(chars.peek(), Some(&(_, '|'))) {
                    chars.next();
                    Token {
                        kind: TokenKind::DoublePipe,
                        position: idx,
                    }
                } else {
                    return Err(SyntaxError::new(format!(
                        "unexpected character `|` at position {idx}; did you mean `||`?"
                    )));
                }
            }
            '<' => {
                chars.next();
                if matches!(chars.peek(), Some(&(_, '='))) {
                    chars.next();
                    Token {
                        kind: TokenKind::LessEqual,
                        position: idx,
                    }
                } else {
                    Token {
                        kind: TokenKind::Less,
                        position: idx,
                    }
                }
            }
            '>' => {
                chars.next();
                if matches!(chars.peek(), Some(&(_, '='))) {
                    chars.next();
                    Token {
                        kind: TokenKind::GreaterEqual,
                        position: idx,
                    }
                } else {
                    Token {
                        kind: TokenKind::Greater,
                        position: idx,
                    }
                }
            }
            '"' => {
                chars.next();
                let mut value = String::new();
                let mut terminated = false;
                while let Some((next_idx, next_ch)) = chars.next() {
                    match next_ch {
                        '"' => {
                            terminated = true;
                            break;
                        }
                        '\\' => {
                            if let Some((_, escaped)) = chars.next() {
                                match escaped {
                                    'n' => value.push('\n'),
                                    't' => value.push('\t'),
                                    '\\' => value.push('\\'),
                                    '"' => value.push('"'),
                                    other => value.push(other),
                                }
                            } else {
                                return Err(SyntaxError::new(format!(
                                    "unterminated escape sequence starting at position {next_idx}"
                                )));
                            }
                        }
                        other => value.push(other),
                    }
                }
                if !terminated {
                    return Err(SyntaxError::new(format!(
                        "unterminated string literal starting at position {idx}"
                    )));
                }
                Token {
                    kind: TokenKind::StringLiteral(value),
                    position: idx,
                }
            }
            ch if ch.is_ascii_digit() => {
                let mut has_dot = false;
                while let Some(&(_, next_ch)) = chars.peek() {
                    if next_ch.is_ascii_digit() {
                        chars.next();
                    } else if next_ch == '.' && !has_dot {
                        has_dot = true;
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
                if literal.ends_with('.') {
                    return Err(SyntaxError::new(format!(
                        "invalid float literal `{literal}` at position {idx}"
                    )));
                }
                if has_dot {
                    let value = literal.parse::<f64>().map_err(|err| {
                        SyntaxError::new(format!(
                            "failed to parse float literal `{literal}` at position {idx}: {err}"
                        ))
                    })?;
                    Token {
                        kind: TokenKind::FloatLiteral(value),
                        position: idx,
                    }
                } else {
                    let value = literal.parse::<i64>().map_err(|err| {
                        SyntaxError::new(format!(
                            "failed to parse integer literal `{literal}` at position {idx}: {err}"
                        ))
                    })?;
                    Token {
                        kind: TokenKind::IntegerLiteral(value),
                        position: idx,
                    }
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
                let lowered = ident.to_lowercase();
                let kind = match lowered.as_str() {
                    "true" => TokenKind::BoolLiteral(true),
                    "false" => TokenKind::BoolLiteral(false),
                    _ => TokenKind::Identifier(ident),
                };
                Token {
                    kind,
                    position: idx,
                }
            }
            _ => {
                return Err(SyntaxError::new(format!(
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
    fn lexes_mutable_variable_declaration() {
        let tokens = lex("int! value = 10;").unwrap();
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[0].kind, TokenKind::Identifier(ref name) if name == "int"));
        assert!(matches!(tokens[1].kind, TokenKind::Bang));
        assert!(matches!(tokens[2].kind, TokenKind::Identifier(ref name) if name == "value"));
        assert!(matches!(tokens[3].kind, TokenKind::Equals));
        assert!(matches!(tokens[4].kind, TokenKind::IntegerLiteral(10)));
        assert!(matches!(tokens[5].kind, TokenKind::Semicolon));
    }

    #[test]
    fn lexes_expression_tokens() {
        let tokens = lex("a = (b + 3.5) * -2 != 0 && true;").unwrap();
        assert!(tokens
            .iter()
            .any(|t| matches!(t.kind, TokenKind::FloatLiteral(_))));
        assert!(tokens
            .iter()
            .any(|t| matches!(t.kind, TokenKind::DoubleAmpersand)));
        assert!(tokens
            .iter()
            .any(|t| matches!(t.kind, TokenKind::BangEqual)));
        assert!(tokens
            .iter()
            .any(|t| matches!(t.kind, TokenKind::BoolLiteral(true))));
    }
}
