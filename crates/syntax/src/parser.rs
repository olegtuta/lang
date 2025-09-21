use crate::ast::{
    Assignment, BinaryOp, Expr, Literal, Statement, TypeAnnotation, UnaryOp, VarDeclaration,
};
use crate::error::{SyntaxError, SyntaxResult};
use crate::lexer::lex;
use crate::token::{Token, TokenKind};

pub fn parse_statement(source: &str) -> SyntaxResult<Statement> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens);
    let statement = parser.parse_statement()?;
    if !parser.is_at_end() {
        return Err(SyntaxError::new("unexpected tokens after end of statement"));
    }
    Ok(statement)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn parse_statement(&mut self) -> SyntaxResult<Statement> {
        if self.check_identifier("echo") {
            self.advance();
            let expr = self.parse_expression()?;
            self.consume(TokenKind::Semicolon)?;
            Ok(Statement::Echo(expr))
        } else if self.is_var_declaration_start() {
            self.parse_var_declaration()
        } else if matches!(self.peek_kind(), Some(TokenKind::Identifier(_))) {
            self.parse_assignment()
        } else {
            Err(SyntaxError::new(format!(
                "unexpected token {:?} at start of statement",
                self.peek_kind()
            )))
        }
    }

    fn parse_var_declaration(&mut self) -> SyntaxResult<Statement> {
        let annotation = self.parse_type_annotation()?;
        let name = self.consume_identifier()?;
        let value = if self.match_token(TokenKind::Equals) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.consume(TokenKind::Semicolon)?;
        Ok(Statement::VarDeclaration(VarDeclaration::new(
            name, annotation, value,
        )))
    }

    fn parse_assignment(&mut self) -> SyntaxResult<Statement> {
        let name = self.consume_identifier()?;
        self.consume(TokenKind::Equals)?;
        let value = self.parse_expression()?;
        self.consume(TokenKind::Semicolon)?;
        Ok(Statement::Assignment(Assignment::new(name, value)))
    }

    fn parse_type_annotation(&mut self) -> SyntaxResult<TypeAnnotation> {
        let type_name = self.consume_identifier()?;
        let mutable = self.match_token(TokenKind::Bang);
        Ok(TypeAnnotation::new(type_name.to_lowercase(), mutable))
    }

    fn parse_expression(&mut self) -> SyntaxResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> SyntaxResult<Expr> {
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

    fn parse_and(&mut self) -> SyntaxResult<Expr> {
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

    fn parse_equality(&mut self) -> SyntaxResult<Expr> {
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

    fn parse_comparison(&mut self) -> SyntaxResult<Expr> {
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

    fn parse_term(&mut self) -> SyntaxResult<Expr> {
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

    fn parse_factor(&mut self) -> SyntaxResult<Expr> {
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

    fn parse_unary(&mut self) -> SyntaxResult<Expr> {
        if self.match_token(TokenKind::Bang) {
            let right = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(right),
            })
        } else if self.match_token(TokenKind::Minus) {
            let right = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(right),
            })
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> SyntaxResult<Expr> {
        if self.is_at_end() {
            return Err(SyntaxError::new("unexpected end of input"));
        }
        self.advance();
        let token = self
            .previous()
            .expect("advance ensures previous token exists")
            .clone();

        match token.kind {
            TokenKind::IntegerLiteral(value) => Ok(Expr::Literal(Literal::Integer(value))),
            TokenKind::FloatLiteral(value) => Ok(Expr::Literal(Literal::Float(value))),
            TokenKind::StringLiteral(value) => Ok(Expr::Literal(Literal::Str(value))),
            TokenKind::BoolLiteral(value) => Ok(Expr::Literal(Literal::Bool(value))),
            TokenKind::Identifier(name) => Ok(Expr::Variable(name)),
            TokenKind::LParen => {
                let expr = self.parse_expression()?;
                self.consume(TokenKind::RParen)?;
                Ok(expr)
            }
            other => Err(SyntaxError::new(format!(
                "unexpected token {:?} in expression",
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

    fn consume(&mut self, kind: TokenKind) -> SyntaxResult<&Token> {
        if self.check(&kind) {
            Ok(self.advance().expect("token should exist"))
        } else {
            Err(SyntaxError::new(format!(
                "expected token {:?}, found {:?}",
                kind,
                self.peek_kind()
            )))
        }
    }

    fn consume_identifier(&mut self) -> SyntaxResult<String> {
        match self.advance().map(|token| &token.kind) {
            Some(TokenKind::Identifier(name)) => Ok(name.clone()),
            Some(TokenKind::StringLiteral(_)) => Err(SyntaxError::new(
                "string literal cannot appear where identifier is expected",
            )),
            Some(other) => Err(SyntaxError::new(format!(
                "expected identifier, found {:?}",
                other
            ))),
            None => Err(SyntaxError::new(
                "unexpected end of input while reading identifier",
            )),
        }
    }

    fn check_identifier(&self, expected: &str) -> bool {
        match self.peek_kind() {
            Some(TokenKind::Identifier(name)) => name == expected,
            _ => false,
        }
    }

    fn is_var_declaration_start(&self) -> bool {
        match self.peek_kind() {
            Some(TokenKind::Identifier(_)) => match self.peek_kind_at(1) {
                Some(TokenKind::Bang) => {
                    matches!(self.peek_kind_at(2), Some(TokenKind::Identifier(_)))
                }
                Some(TokenKind::Identifier(_)) => true,
                _ => false,
            },
            _ => false,
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        matches!(self.peek_kind(), Some(token_kind) if token_kind == kind)
    }

    fn advance(&mut self) -> Option<&Token> {
        if !self.is_at_end() {
            self.position += 1;
        }
        self.previous()
    }

    fn previous(&self) -> Option<&Token> {
        if self.position == 0 {
            None
        } else {
            self.tokens.get(self.position - 1)
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|token| &token.kind)
    }

    fn peek_kind_at(&self, offset: usize) -> Option<&TokenKind> {
        self.tokens
            .get(self.position + offset)
            .map(|token| &token.kind)
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_echo_statement() {
        let stmt = parse_statement("echo 1 + 2;").unwrap();
        assert!(matches!(stmt, Statement::Echo(_)));
    }

    #[test]
    fn parse_mutable_declaration() {
        let stmt = parse_statement("int! value = 10;").unwrap();
        if let Statement::VarDeclaration(decl) = stmt {
            assert_eq!(decl.ty.name, "int");
            assert!(decl.ty.mutable);
        } else {
            panic!("expected var declaration");
        }
    }

    #[test]
    fn parse_immutable_declaration_without_initializer() {
        let stmt = parse_statement("int count;").unwrap();
        if let Statement::VarDeclaration(decl) = stmt {
            assert_eq!(decl.ty.name, "int");
            assert!(!decl.ty.mutable);
            assert!(decl.value.is_none());
        } else {
            panic!("expected var declaration");
        }
    }
}
