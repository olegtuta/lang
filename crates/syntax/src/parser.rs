use crate::ast::{
    ArrayElement, Assignment, AssignmentKind, AssignmentTarget, BinaryOp, ElseBranch, Expr,
    IfStatement, IncrementOp, IndexTarget, Literal, Statement, TypeAnnotation, UnaryOp,
    VarDeclaration, WhileStatement,
};
use crate::error::{SyntaxError, SyntaxResult};
use crate::lexer::lex;
use crate::token::{Token, TokenKind};

pub fn parse_statement(source: &str) -> SyntaxResult<Statement> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens);
    parser.skip_newlines();
    let statement = parser.statement()?;
    parser.skip_newlines();
    if !parser.is_at_end() {
        return Err(SyntaxError::new("unexpected tokens after end of statement"));
    }
    Ok(statement)
}

pub fn parse_program(source: &str) -> SyntaxResult<Vec<Statement>> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens);
    parser.parse_program()
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

    fn parse_program(&mut self) -> SyntaxResult<Vec<Statement>> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            self.skip_newlines();
            if self.is_at_end() {
                break;
            }
            let stmt = self.statement()?;
            statements.push(stmt);
            self.skip_newlines();
        }
        Ok(statements)
    }

    fn statement(&mut self) -> SyntaxResult<Statement> {
        if self.match_token(TokenKind::Echo) {
            let expr = self.parse_expression()?;
            Ok(Statement::Echo(expr))
        } else if self.match_token(TokenKind::Let) {
            self.parse_let_declaration()
        } else if self.match_token(TokenKind::If) {
            let if_stmt = self.parse_if_core()?;
            Ok(Statement::If(if_stmt))
        } else if self.match_token(TokenKind::While) {
            let condition = self.parenthesized_expression()?;
            let body = self.parse_block()?;
            Ok(Statement::While(WhileStatement::new(condition, body)))
        } else if self.match_token(TokenKind::Break) {
            Ok(Statement::Break)
        } else if self.match_token(TokenKind::Continue) {
            Ok(Statement::Continue)
        } else if matches!(self.peek_kind(), Some(TokenKind::Identifier(_))) {
            self.parse_assignment_statement()
        } else {
            Err(SyntaxError::new(format!(
                "unexpected token {:?} at start of statement",
                self.peek_kind()
            )))
        }
    }

    fn parse_block(&mut self) -> SyntaxResult<Vec<Statement>> {
        self.consume(TokenKind::LBrace)?;
        let mut statements = Vec::new();
        loop {
            self.skip_newlines();
            if self.match_token(TokenKind::RBrace) {
                break;
            }
            if self.is_at_end() {
                return Err(SyntaxError::new("unterminated block"));
            }
            let stmt = self.statement()?;
            statements.push(stmt);
            self.skip_newlines();
        }
        Ok(statements)
    }

    fn parse_if_core(&mut self) -> SyntaxResult<IfStatement> {
        let condition = self.parenthesized_expression()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.match_token(TokenKind::Else) {
            if self.match_token(TokenKind::If) {
                let nested = self.parse_if_core()?;
                Some(Box::new(ElseBranch::If(nested)))
            } else {
                let block = self.parse_block()?;
                Some(Box::new(ElseBranch::Block(block)))
            }
        } else {
            None
        };
        Ok(IfStatement::new(condition, then_branch, else_branch))
    }

    fn parse_assignment_statement(&mut self) -> SyntaxResult<Statement> {
        let name = self.consume_identifier()?;
        let indices = self.parse_index_chain()?;
        if indices.is_empty() {
            self.parse_assignment_for_name(name)
        } else {
            self.parse_index_assignment(name, indices)
        }
    }

    fn parse_assignment_for_name(&mut self, name: String) -> SyntaxResult<Statement> {
        if self.match_token(TokenKind::PlusPlus) {
            return Ok(Statement::Assignment(Assignment::new(
                AssignmentTarget::Name(name),
                AssignmentKind::Increment(IncrementOp::Increment),
            )));
        }
        if self.match_token(TokenKind::MinusMinus) {
            return Ok(Statement::Assignment(Assignment::new(
                AssignmentTarget::Name(name),
                AssignmentKind::Increment(IncrementOp::Decrement),
            )));
        }
        if self.match_token(TokenKind::PlusEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                AssignmentTarget::Name(name),
                AssignmentKind::Compound {
                    op: BinaryOp::Add,
                    expr,
                },
            )));
        }
        if self.match_token(TokenKind::MinusEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                AssignmentTarget::Name(name),
                AssignmentKind::Compound {
                    op: BinaryOp::Subtract,
                    expr,
                },
            )));
        }
        if self.match_token(TokenKind::StarEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                AssignmentTarget::Name(name),
                AssignmentKind::Compound {
                    op: BinaryOp::Multiply,
                    expr,
                },
            )));
        }
        if self.match_token(TokenKind::SlashEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                AssignmentTarget::Name(name),
                AssignmentKind::Compound {
                    op: BinaryOp::Divide,
                    expr,
                },
            )));
        }
        if self.match_token(TokenKind::PercentEquals) {
            let expr = self.parse_expression()?;
            return Ok(Statement::Assignment(Assignment::new(
                AssignmentTarget::Name(name),
                AssignmentKind::Compound {
                    op: BinaryOp::Modulo,
                    expr,
                },
            )));
        }
        self.consume(TokenKind::Equals)?;
        let expr = self.parse_expression()?;
        Ok(Statement::Assignment(Assignment::new(
            AssignmentTarget::Name(name),
            AssignmentKind::Simple(expr),
        )))
    }

    fn parse_index_assignment(
        &mut self,
        name: String,
        indices: Vec<IndexTarget>,
    ) -> SyntaxResult<Statement> {
        if self.match_token(TokenKind::PlusPlus)
            || self.match_token(TokenKind::MinusMinus)
            || self.match_token(TokenKind::PlusEquals)
            || self.match_token(TokenKind::MinusEquals)
            || self.match_token(TokenKind::StarEquals)
            || self.match_token(TokenKind::SlashEquals)
            || self.match_token(TokenKind::PercentEquals)
        {
            return Err(SyntaxError::new(
                "compound and increment assignments are not supported for indexed targets",
            ));
        }
        self.consume(TokenKind::Equals)?;
        let expr = self.parse_expression()?;
        Ok(Statement::Assignment(Assignment::new(
            AssignmentTarget::Indexed { name, indices },
            AssignmentKind::Simple(expr),
        )))
    }

    fn parse_index_chain(&mut self) -> SyntaxResult<Vec<IndexTarget>> {
        let mut indices = Vec::new();
        loop {
            if self.match_token(TokenKind::LBracket) {
                if self.match_token(TokenKind::RBracket) {
                    indices.push(IndexTarget::Append);
                } else {
                    let expr = self.parse_expression()?;
                    self.consume(TokenKind::RBracket)?;
                    indices.push(IndexTarget::Index(expr));
                }
            } else {
                break;
            }
        }
        Ok(indices)
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

    fn parse_type_annotation(&mut self) -> SyntaxResult<TypeAnnotation> {
        let type_name = self.consume_identifier()?.to_lowercase();
        let generics = if self.match_token(TokenKind::Less) {
            let mut params = Vec::new();
            loop {
                params.push(self.parse_type_annotation()?);
                if self.match_token(TokenKind::Comma) {
                    continue;
                }
                self.consume(TokenKind::Greater)?;
                break;
            }
            params
        } else {
            Vec::new()
        };
        Ok(TypeAnnotation::with_generics(type_name, generics))
    }

    fn parenthesized_expression(&mut self) -> SyntaxResult<Expr> {
        self.consume(TokenKind::LParen)?;
        let expr = self.parse_expression()?;
        self.consume(TokenKind::RParen)?;
        Ok(expr)
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
            let expr = self.parse_primary()?;
            self.parse_postfix(expr)
        }
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> SyntaxResult<Expr> {
        loop {
            if self.match_token(TokenKind::LBracket) {
                if self.check(&TokenKind::RBracket) {
                    return Err(SyntaxError::new(
                        "empty index accessor is only allowed in assignment context",
                    ));
                }
                let index = self.parse_expression()?;
                self.consume(TokenKind::RBracket)?;
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(index),
                };
            } else {
                break;
            }
        }
        Ok(expr)
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
            TokenKind::LBracket => {
                let mut elements = Vec::new();
                if self.match_token(TokenKind::RBracket) {
                    return Ok(Expr::Literal(Literal::Array(Vec::new())));
                }
                loop {
                    let expr = self.parse_expression()?;
                    if self.match_token(TokenKind::Arrow) {
                        let value = self.parse_expression()?;
                        elements.push(ArrayElement::KeyValue { key: expr, value });
                    } else {
                        elements.push(ArrayElement::Value(expr));
                    }
                    if self.match_token(TokenKind::Comma) {
                        continue;
                    }
                    self.consume(TokenKind::RBracket)?;
                    break;
                }
                Ok(Expr::Literal(Literal::Array(elements)))
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
        matches!(self.peek_kind(), Some(token_kind) if token_kind == *kind)
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

    fn peek_kind(&self) -> Option<TokenKind> {
        self.tokens
            .get(self.position)
            .map(|token| token.kind.clone())
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_echo_statement() {
        let stmt = parse_statement("echo 1 + 2").unwrap();
        assert!(matches!(stmt, Statement::Echo(_)));
    }

    #[test]
    fn parses_let_declaration_with_type() {
        let stmt = parse_statement("let fix value: int = 10").unwrap();
        match stmt {
            Statement::Let(decl) => {
                assert_eq!(decl.name, "value");
                assert!(!decl.mutable);
                assert_eq!(decl.ty.unwrap().name, "int");
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn parses_assignment_with_index() {
        let stmt = parse_statement("array[0] = 42").unwrap();
        match stmt {
            Statement::Assignment(Assignment { target, .. }) => match target {
                AssignmentTarget::Indexed { name, indices } => {
                    assert_eq!(name, "array");
                    assert_eq!(indices.len(), 1);
                }
                _ => panic!("expected indexed target"),
            },
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn parses_if_else_statement() {
        let source = "if (x > 1) { echo 1 } else { echo 2 }";
        let stmt = parse_statement(source).unwrap();
        assert!(matches!(stmt, Statement::If(_)));
    }

    #[test]
    fn parses_array_literal() {
        let stmt = parse_statement("echo [1, 2 => 3]").unwrap();
        match stmt {
            Statement::Echo(Expr::Literal(Literal::Array(elements))) => {
                assert_eq!(elements.len(), 2);
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }
}
