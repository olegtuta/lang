use lang_core::{LangError, LangResult, LangType, TypeRegistry, Value};

use crate::ast::VarDeclaration;
use crate::lexer::{lex, Token, TokenKind};

pub fn parse_variable_declaration(
    source: &str,
    registry: &TypeRegistry,
) -> LangResult<VarDeclaration> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens, registry);
    parser.parse_variable_declaration()
}

struct Parser<'a> {
    tokens: Vec<Token>,
    position: usize,
    registry: &'a TypeRegistry,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token>, registry: &'a TypeRegistry) -> Self {
        Self {
            tokens,
            position: 0,
            registry,
        }
    }

    fn parse_variable_declaration(&mut self) -> LangResult<VarDeclaration> {
        self.consume(TokenKind::LBracket)?;
        let (ty, mutable) = self.parse_type_annotation()?;
        self.consume(TokenKind::RBracket)?;
        self.consume(TokenKind::Dollar)?;
        let name = self.consume_identifier()?;
        let lang_type = LangType::new(ty, mutable);
        let value = if self.matches(TokenKind::Equals) {
            self.advance();
            Some(self.parse_literal(&lang_type)?)
        } else {
            None
        };
        self.consume(TokenKind::Semicolon)?;
        Ok(VarDeclaration::new(name, lang_type, value))
    }

    fn parse_type_annotation(&mut self) -> LangResult<(lang_core::TypeKind, bool)> {
        let mut type_name: Option<String> = None;
        let mut mutable = false;

        loop {
            let ident = self.consume_identifier()?;
            let lowered = ident.to_lowercase();
            if lowered == "mute" {
                mutable = true;
            } else if type_name.is_none() {
                type_name = Some(lowered);
            } else {
                return Err(LangError::parse(format!(
                    "duplicate type identifier `{}` in annotation",
                    ident
                )));
            }

            if self.matches(TokenKind::Comma) {
                self.advance();
                continue;
            }
            break;
        }

        let type_name = type_name
            .ok_or_else(|| LangError::parse("type annotation must include a base type"))?;

        let ty = self
            .registry
            .resolve(&type_name)
            .ok_or_else(|| LangError::unknown_type(type_name.clone()))?;

        Ok((ty, mutable))
    }

    fn parse_literal(&mut self, expected_type: &LangType) -> LangResult<Value> {
        match expected_type.kind() {
            lang_core::TypeKind::Primitive(lang_core::PrimitiveType::Integer) => {
                if let Some(TokenKind::IntegerLiteral(value)) = self.peek_kind().cloned() {
                    self.advance();
                    Ok(Value::from(value))
                } else {
                    Err(LangError::parse(format!(
                        "expected integer literal, found {:?}",
                        self.peek_kind()
                    )))
                }
            }
        }
    }

    fn consume(&mut self, expected: TokenKind) -> LangResult<()> {
        let actual = self.peek_kind().cloned();
        if actual.as_ref() == Some(&expected) {
            self.advance();
            return Ok(());
        }
        Err(LangError::parse(format!(
            "expected token {:?}, found {:?}",
            expected, actual
        )))
    }

    fn consume_identifier(&mut self) -> LangResult<String> {
        match self.peek_kind() {
            Some(TokenKind::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            other => Err(LangError::parse(format!(
                "expected identifier, found {:?}",
                other
            ))),
        }
    }

    fn matches(&self, kind: TokenKind) -> bool {
        matches!(self.peek_kind(), Some(k) if *k == kind)
    }

    fn advance(&mut self) {
        self.position += 1;
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.position).map(|token| &token.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> TypeRegistry {
        TypeRegistry::new()
    }

    #[test]
    fn parses_integer_declaration_with_value() {
        let decl = parse_variable_declaration("[int] $value = 10;", &registry()).unwrap();
        assert_eq!(decl.name, "value");
        assert_eq!(decl.ty, LangType::integer());
        assert_eq!(decl.value.unwrap().expect_integer().unwrap(), 10);
    }

    #[test]
    fn parses_integer_declaration_without_value() {
        let decl = parse_variable_declaration("[int] $value;", &registry()).unwrap();
        assert_eq!(decl.name, "value");
        assert!(decl.value.is_none());
    }

    #[test]
    fn parses_mutable_integer_declaration_with_any_order() {
        let decl = parse_variable_declaration("[mute, int] $value = 1;", &registry()).unwrap();
        assert!(decl.ty.is_mutable());
        assert_eq!(decl.value.unwrap().expect_integer().unwrap(), 1);
    }
}
