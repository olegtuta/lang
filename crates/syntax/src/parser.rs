use crate::ast::{
    Assignment, AssignmentKind, BinaryOp, Expr, IncrementOp, Literal, Statement, TypeAnnotation,
    UnaryOp, VarDeclaration,
};
use crate::error::{SyntaxError, SyntaxResult};
use crate::lexer::lex;
use crate::token::{Token, TokenKind};

pub fn parse_statement(source: &str) -> SyntaxResult<Statement> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens);
    parser.skip_newlines();
    let statement = parser.parse_statement()?;
    parser.skip_newlines();
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
        if self.match_token(TokenKind::Echo) {
            let expr = self.parse_expression()?;
            Ok(Statement::Echo(expr))
        } else if self.match_token(TokenKind::Let) {
            self.parse_let_declaration()
        } else if matches!(self.peek_kind(), Some(TokenKind::Identifier(_))) {
            self.parse_assignment()
        } else {
            Err(SyntaxError::new(format!(
                "unexpected token {:?} at start of statement",
                self.peek_kind()
            )))
        }
    }

    fn parse_let_declaration(&mut self) -> SyntaxResult<Statement> {
        let mutable = if self.match_token(TokenKind::Fix) {
            false
        } else {
            true
        };
        let name = self.consume_identifier()?;
        let ty = if self.match_token(TokenKind::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };
        let value = if self.match_token(TokenKind::Equals) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        Ok(Statement::Let(VarDeclaration::new(
            name, ty, mutable, value,
        )))
    }

    fn parse_assignment(&mut self) -> SyntaxResult<Statement> {
        let name = self.consume_identifier()?;
        if self.match_token(TokenKind::PlusPlus) {
            return Ok(Statement::Assignment(Assignment::new(
                name,
                AssignmentKind::Increment(IncrementOp::Increment),
            )));
        }
        if self.match_token(TokenKind::MinusMinus) {
            return Ok(Statement::Assignment(Assignment::new(
                name,
                AssignmentKind::Increment(IncrementOp::Decrement),
            )));
        }

        if self.match_token(TokenKind::PlusEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                name,
                AssignmentKind::Compound {
                    op: BinaryOp::Add,
                    expr,
                },
            )));
        }
        if self.match_token(TokenKind::MinusEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                name,
                AssignmentKind::Compound {
                    op: BinaryOp::Subtract,
                    expr,
                },
            )));
        }
        if self.match_token(TokenKind::StarEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                name,
                AssignmentKind::Compound {
                    op: BinaryOp::Multiply,
                    expr,
                },
            )));
        }
        if self.match_token(TokenKind::SlashEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                name,
                AssignmentKind::Compound {
                    op: BinaryOp::Divide,
                    expr,
                },
            )));
        }
        if self.match_token(TokenKind::PercentEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                name,
                AssignmentKind::Compound {
                    op: BinaryOp::Modulo,
                    expr,
                },
            )));
        }

        self.consume(TokenKind::Equals)?;
        let expr = self.parse_expression()?;
        Ok(Statement::Assignment(Assignment::new(
            name,
            AssignmentKind::Simple(expr),
        )))
    }

    fn parse_type_annotation(&mut self) -> SyntaxResult<TypeAnnotation> {
        let type_name = self.consume_identifier()?;
        Ok(TypeAnnotation::new(type_name.to_lowercase()))
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

    fn check(&self, kind: &TokenKind) -> bool {
        matches!(self.peek_kind(), Some(token_kind) if token_kind == kind)
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek_kind(), Some(TokenKind::Newline)) {
            self.advance();
        }
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

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_echo_statement() {
        let stmt = parse_statement("echo 1 + 2").unwrap();
        assert!(matches!(stmt, Statement::Echo(_)));
    }

    #[test]
    fn parse_mutable_declaration() {
        let stmt = parse_statement("let value: int = 10").unwrap();
        if let Statement::Let(decl) = stmt {
            assert_eq!(decl.ty.as_ref().unwrap().name, "int");
            assert!(decl.mutable);
        } else {
            panic!("expected var declaration");
        }
    }

    #[test]
    fn parse_immutable_declaration_without_initializer() {
        let stmt = parse_statement("let fix count: int").unwrap();
        if let Statement::Let(decl) = stmt {
            assert_eq!(decl.ty.as_ref().unwrap().name, "int");
            assert!(!decl.mutable);
            assert!(decl.value.is_none());
        } else {
            panic!("expected var declaration");
        }
    }

    #[test]
    fn parse_declaration_without_type_defaults_to_mixed() {
        let stmt = parse_statement("let value").unwrap();
        if let Statement::Let(decl) = stmt {
            assert!(decl.ty.is_none());
            assert!(decl.mutable);
        } else {
            panic!("expected var declaration");
        }
    }
}
