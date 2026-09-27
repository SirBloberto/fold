use super::header::Header;
use super::token::Pos;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub header: Header,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Input {
        name: String,
        range: Option<(Expr, Expr)>,
        default: Expr,
    },
    Let {
        name: String,
        value: Expr,
    },
    Func {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    Return(Expr),
    Draw(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Num(f32),
    Rgba(u32),
    Name(String),
    Negate(Box<Expr>),
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Field {
        target: Box<Expr>,
        field: String,
    },
    Lambda {
        param: String,
        body: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}
