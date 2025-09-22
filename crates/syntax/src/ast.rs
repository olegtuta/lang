#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Let(VarDeclaration),
    Assignment(Assignment),
    Echo(Expr),
    If(IfStatement),
    While(WhileStatement),
    Break,
    Continue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    pub name: String,
    pub generics: Vec<TypeAnnotation>,
}

impl TypeAnnotation {
    pub fn new(name: String) -> Self {
        Self {
            name,
            generics: Vec::new(),
        }
    }

    pub fn with_generics(name: String, generics: Vec<TypeAnnotation>) -> Self {
        Self { name, generics }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDeclaration {
    pub name: String,
    pub ty: Option<TypeAnnotation>,
    pub mutable: bool,
    pub value: Option<Expr>,
}

impl VarDeclaration {
    pub fn new(
        name: String,
        ty: Option<TypeAnnotation>,
        mutable: bool,
        value: Option<Expr>,
    ) -> Self {
        Self {
            name,
            ty,
            mutable,
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub target: AssignmentTarget,
    pub kind: AssignmentKind,
}

impl Assignment {
    pub fn new(target: AssignmentTarget, kind: AssignmentKind) -> Self {
        Self { target, kind }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentTarget {
    Name(String),
    Indexed {
        name: String,
        indices: Vec<IndexTarget>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndexTarget {
    Index(Expr),
    Append,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentKind {
    Simple(Expr),
    Compound { op: BinaryOp, expr: Expr },
    Increment(IncrementOp),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(Vec<ArrayElement>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Variable(String),
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },
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

#[derive(Debug, Clone, PartialEq)]
pub struct IfStatement {
    pub condition: Expr,
    pub then_branch: Vec<Statement>,
    pub else_branch: Option<Box<ElseBranch>>,
}

impl IfStatement {
    pub fn new(
        condition: Expr,
        then_branch: Vec<Statement>,
        else_branch: Option<Box<ElseBranch>>,
    ) -> Self {
        Self {
            condition,
            then_branch,
            else_branch,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElseBranch {
    If(IfStatement),
    Block(Vec<Statement>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhileStatement {
    pub condition: Expr,
    pub body: Vec<Statement>,
}

impl WhileStatement {
    pub fn new(condition: Expr, body: Vec<Statement>) -> Self {
        Self { condition, body }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayElement {
    Value(Expr),
    KeyValue { key: Expr, value: Expr },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncrementOp {
    Increment,
    Decrement,
}
