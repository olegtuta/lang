use lang_core::{LangError, LangResult, LangType, TypeKind, TypeRegistry, Value};

use crate::ast::{Assignment, BinaryOp, Expr, Statement, UnaryOp, VarDeclaration};
use crate::lexer::{lex, Token, TokenKind};

pub fn parse_statement(source: &str, registry: &TypeRegistry) -> LangResult<Statement> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens, registry);
    let statement = parser.parse_statement()?;
    if !parser.is_at_end() {
        return Err(LangError::parse("unexpected tokens after end of statement"));
    }
    Ok(statement)
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

    fn parse_statement(&mut self) -> LangResult<Statement> {
        if self.check(&TokenKind::LBracket) {
            self.parse_var_declaration()
        } else if self.check_identifier("echo") {
            self.advance();
            let expr = self.parse_expression()?;
            self.consume(TokenKind::Semicolon)?;
            Ok(Statement::Echo(expr))
        } else if self.check(&TokenKind::Dollar) {
            self.parse_assignment()
        } else {
            Err(LangError::parse(format!(
                "unexpected token {:?} at start of statement",
                self.peek_kind()
            )))
        }
    }

    fn parse_var_declaration(&mut self) -> LangResult<Statement> {
        self.consume(TokenKind::LBracket)?;
        let (kind, mutable) = self.parse_type_annotation()?;
        self.consume(TokenKind::RBracket)?;
        self.consume(TokenKind::Dollar)?;
        let name = self.consume_identifier()?;
        let lang_type = LangType::new(kind, mutable);
        let value = if self.match_token(TokenKind::Equals) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.consume(TokenKind::Semicolon)?;
        Ok(Statement::VarDeclaration(VarDeclaration::new(
            name, lang_type, value,
        )))
    }

    fn parse_assignment(&mut self) -> LangResult<Statement> {
        self.consume(TokenKind::Dollar)?;
        let name = self.consume_identifier()?;
        self.consume(TokenKind::Equals)?;
        let value = self.parse_expression()?;
        self.consume(TokenKind::Semicolon)?;
        Ok(Statement::Assignment(Assignment::new(name, value)))
    }

    fn parse_type_annotation(&mut self) -> LangResult<(TypeKind, bool)> {
        let mut type_name: Option<String> = None;
        let mut mutable = false;

        loop {
            let ident = self.consume_identifier()?;
            let lowered = ident.to_lowercase();
            if lowered == "mut" || lowered == "mute" {
                if mutable {
                    return Err(LangError::parse("mutability specified more than once"));
                }
                mutable = true;
            } else if type_name.is_none() {
                type_name = Some(lowered);
            } else {
                return Err(LangError::parse(format!(
                    "duplicate type identifier `{ident}` in annotation"
                )));
            }

            if self.match_token(TokenKind::Comma) {
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

    fn parse_expression(&mut self) -> LangResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> LangResult<Expr> {
        let mut expr = self.parse_and()?;
        while self.match_token(TokenKind::DoublePipe) {
            let right = self.parse_and()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Or,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> LangResult<Expr> {
        let mut expr = self.parse_equality()?;
        while self.match_token(TokenKind::DoubleAmpersand) {
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::And,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> LangResult<Expr> {
        let mut expr = self.parse_comparison()?;
        loop {
            if self.match_token(TokenKind::EqualEqual) {
                let right = self.parse_comparison()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Equal,
                    right: Box::new(right),
                };
            } else if self.match_token(TokenKind::BangEqual) {
                let right = self.parse_comparison()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::NotEqual,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> LangResult<Expr> {
        let mut expr = self.parse_term()?;
        loop {
            if self.match_token(TokenKind::Less) {
                let right = self.parse_term()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Less,
                    right: Box::new(right),
                };
            } else if self.match_token(TokenKind::LessEqual) {
                let right = self.parse_term()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::LessEqual,
                    right: Box::new(right),
                };
            } else if self.match_token(TokenKind::Greater) {
                let right = self.parse_term()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Greater,
                    right: Box::new(right),
                };
            } else if self.match_token(TokenKind::GreaterEqual) {
                let right = self.parse_term()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::GreaterEqual,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> LangResult<Expr> {
        let mut expr = self.parse_factor()?;
        loop {
            if self.match_token(TokenKind::Plus) {
                let right = self.parse_factor()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Add,
                    right: Box::new(right),
                };
            } else if self.match_token(TokenKind::Minus) {
                let right = self.parse_factor()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Subtract,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> LangResult<Expr> {
        let mut expr = self.parse_unary()?;
        loop {
            if self.match_token(TokenKind::Star) {
                let right = self.parse_unary()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Multiply,
                    right: Box::new(right),
                };
            } else if self.match_token(TokenKind::Slash) {
                let right = self.parse_unary()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Divide,
                    right: Box::new(right),
                };
            } else if self.match_token(TokenKind::Percent) {
                let right = self.parse_unary()?;
                expr = Expr::Binary {
                    left: Box::new(expr),
                    op: BinaryOp::Modulo,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> LangResult<Expr> {
        if self.match_token(TokenKind::Bang) {
            let expr = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            })
        } else if self.match_token(TokenKind::Minus) {
            let expr = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(expr),
            })
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> LangResult<Expr> {
        match self.peek_kind() {
            Some(TokenKind::IntegerLiteral(value)) => {
                let value = *value;
                self.advance();
                Ok(Expr::Literal(Value::from(value)))
            }
            Some(TokenKind::FloatLiteral(value)) => {
                let value = *value;
                self.advance();
                Ok(Expr::Literal(Value::from(value)))
            }
            Some(TokenKind::StringLiteral(value)) => {
                let value = value.clone();
                self.advance();
                Ok(Expr::Literal(Value::from(value)))
            }
            Some(TokenKind::BoolLiteral(value)) => {
                let value = *value;
                self.advance();
                Ok(Expr::Literal(Value::from(value)))
            }
            Some(TokenKind::Dollar) => {
                self.advance();
                let name = self.consume_identifier()?;
                Ok(Expr::Variable(name))
            }
            Some(TokenKind::LParen) => {
                self.advance();
                let expr = self.parse_expression()?;
                self.consume(TokenKind::RParen)?;
                Ok(expr)
            }
            Some(TokenKind::Identifier(name)) => Err(LangError::parse(format!(
                "unexpected identifier `{name}`; variables must be referenced as `$name`"
            ))),
            other => Err(LangError::parse(format!(
                "expected expression, found {:?}",
                other
            ))),
        }
    }

    fn consume(&mut self, expected: TokenKind) -> LangResult<()> {
        let actual = self.peek_kind().cloned();
        if actual.as_ref() == Some(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(LangError::parse(format!(
                "expected token {:?}, found {:?}",
                expected, actual
            )))
        }
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

    fn match_token(&mut self, expected: TokenKind) -> bool {
        if self.check(&expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, expected: &TokenKind) -> bool {
        self.peek_kind()
            .map(|kind| kind == expected)
            .unwrap_or(false)
    }

    fn check_identifier(&self, expected: &str) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Identifier(name)) if name.eq_ignore_ascii_case(expected))
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.position += 1;
        }
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.position).map(|token| &token.kind)
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> TypeRegistry {
        TypeRegistry::new()
    }

    #[test]
    fn parses_integer_declaration_with_expression() {
        let stmt = parse_statement("[int, mut] $value = 10 + 2;", &registry()).unwrap();
        match stmt {
            Statement::VarDeclaration(decl) => {
                assert!(decl.ty.is_mutable());
                assert!(decl.value.is_some());
            }
            other => panic!("unexpected statement: {:?}", other),
        }
    }

    #[test]
    fn parses_assignment_statement() {
        let stmt = parse_statement("$value = $other * 3;", &registry()).unwrap();
        match stmt {
            Statement::Assignment(assign) => {
                assert_eq!(assign.name, "value");
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parses_echo_statement() {
        let stmt = parse_statement("echo 1 + 2;", &registry()).unwrap();
        assert!(matches!(stmt, Statement::Echo(_)));
    }

    #[test]
    fn reports_unknown_type() {
        let err = parse_statement("[foo] $value;", &registry()).unwrap_err();
        assert!(format!("{}", err).contains("Unknown type"));
    }
}
