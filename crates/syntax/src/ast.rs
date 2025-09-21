#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    VarDeclaration(VarDeclaration),
    Assignment(Assignment),
    Echo(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    pub name: String,
    pub mutable: bool,
}

impl TypeAnnotation {
    pub fn new(name: String, mutable: bool) -> Self {
        Self { name, mutable }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDeclaration {
    pub name: String,
    pub ty: TypeAnnotation,
    pub value: Option<Expr>,
}

impl VarDeclaration {
    pub fn new(name: String, ty: TypeAnnotation, value: Option<Expr>) -> Self {
        Self { name, ty, value }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub name: String,
    pub value: Expr,
}

impl Assignment {
    pub fn new(name: String, value: Expr) -> Self {
        Self { name, value }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Variable(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}
