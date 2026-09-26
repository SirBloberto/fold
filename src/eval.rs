use crate::pixel::{self, Rgba, Vec2};
use crate::syntax::ast::{BinOp, Expr, ExprKind, Program, Stmt, StmtKind};
use crate::syntax::token::Pos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Num(f32),
    Vec2(Vec2),
    Rgba(Rgba),
}

pub struct Pixel {
    pub p: Vec2,
    pub uv: Vec2,
    pub time: f32,
    pub px: f32,
    pub size: Vec2,
}

pub fn run(program: &Program, pixel: &Pixel) -> Result<Rgba, String> {
    let mut env = Env {
        pixel,
        vars: Vec::new(),
        out: None,
    };
    for stmt in &program.body {
        env.statement(stmt)?;
    }
    env.out.ok_or("this file never sets OUT".into())
}

struct Env<'a> {
    pixel: &'a Pixel,
    vars: Vec<(&'a str, Value)>,
    out: Option<Rgba>,
}

impl<'a> Env<'a> {
    fn statement(&mut self, stmt: &'a Stmt) -> Result<(), String> {
        match &stmt.kind {
            StmtKind::Input { name, default, .. } => {
                let value = self.expr(default)?;
                self.vars.push((name, value));
            }
            StmtKind::Let { name, value } => {
                let value = self.expr(value)?;
                self.vars.push((name, value));
            }
            StmtKind::Assign { name, value } if name == "OUT" => {
                let Value::Rgba(col) = self.expr(value)? else {
                    return Err(at("OUT must be set to a colour", stmt.pos));
                };
                self.out = Some(col);
            }
            StmtKind::Assign { name, .. } => {
                return Err(at(&format!("`{name}` can't be assigned"), stmt.pos));
            }
            StmtKind::Func { .. } | StmtKind::Return(_) => {
                return Err(at("functions aren't supported yet", stmt.pos));
            }
        }
        Ok(())
    }

    fn lookup(&self, name: &str) -> Option<Value> {
        let builtin = match name {
            "P" => Value::Vec2(self.pixel.p),
            "UV" => Value::Vec2(self.pixel.uv),
            "TIME" => Value::Num(self.pixel.time),
            "PX" => Value::Num(self.pixel.px),
            "SIZE" => Value::Vec2(self.pixel.size),
            _ => {
                return self
                    .vars
                    .iter()
                    .rev()
                    .find(|(var, _)| *var == name)
                    .map(|(_, value)| *value);
            }
        };
        Some(builtin)
    }

    fn expr(&self, expr: &Expr) -> Result<Value, String> {
        let value = match &expr.kind {
            ExprKind::Num(n) => Value::Num(*n),
            ExprKind::Rgba(rgb) => Value::Rgba(Rgba::hex(*rgb)),
            ExprKind::Name(name) => match self.lookup(name) {
                Some(value) => value,
                None => return Err(at(&format!("unknown name `{name}`"), expr.pos)),
            },
            ExprKind::Negate(inner) => match self.expr(inner)? {
                Value::Num(n) => Value::Num(-n),
                Value::Vec2(v) => Value::Vec2(Vec2 { x: -v.x, y: -v.y }),
                Value::Rgba(_) => return Err(at("can't negate a colour", expr.pos)),
            },
            ExprKind::Binary { op, left, right } => {
                binary(*op, self.expr(left)?, self.expr(right)?).map_err(|e| at(&e, expr.pos))?
            }
            ExprKind::Call { name, args } => {
                let args = args
                    .iter()
                    .map(|arg| self.expr(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                call(name, &args).map_err(|e| at(&e, expr.pos))?
            }
            ExprKind::Field { target, field } => match (self.expr(target)?, field.as_str()) {
                (Value::Vec2(v), "x") => Value::Num(v.x),
                (Value::Vec2(v), "y") => Value::Num(v.y),
                _ => return Err(at(&format!("no field `{field}`"), expr.pos)),
            },
        };
        Ok(value)
    }
}

fn binary(op: BinOp, left: Value, right: Value) -> Result<Value, String> {
    use BinOp::*;
    use Value::*;

    let result = match (op, left, right) {
        (Add, Num(a), Num(b)) => Num(a + b),
        (Sub, Num(a), Num(b)) => Num(a - b),
        (Mul, Num(a), Num(b)) => Num(a * b),
        (Div, Num(a), Num(b)) => Num(a / b),

        (Sub, Vec2(a), Vec2(b)) => Vec2(a - b),

        (Add, Rgba(a), Rgba(b)) => Rgba(a + b),
        (Mul, Rgba(c), Num(k)) | (Mul, Num(k), Rgba(c)) => Rgba(c * k),

        _ => return Err(format!("can't use {op:?} on {left:?} and {right:?}")),
    };
    Ok(result)
}

fn call(name: &str, args: &[Value]) -> Result<Value, String> {
    use Value::*;

    let result = match (name, args) {
        ("sin", [Num(x)]) => Num(x.sin()),

        ("circle", [Vec2(p), Num(r)]) => Num(pixel::circle(*p, *r)),

        ("fill", [Num(d), Rgba(c)]) => Rgba(pixel::fill(*d, *c)),
        ("glow", [Num(d), Rgba(c), Num(w)]) => Rgba(pixel::glow(*d, *c, *w)),

        _ => return Err(format!("unknown function `{name}`, or wrong arguments")),
    };
    Ok(result)
}

fn at(message: &str, pos: Pos) -> String {
    format!("{message} at line {}, column {}", pos.line, pos.col)
}
