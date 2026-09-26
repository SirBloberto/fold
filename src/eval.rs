use std::collections::HashMap;

use crate::pixel::{self, Colour, Vec2};
use crate::syntax::ast::{BinOp, Expr, ExprKind, Program, Stmt, StmtKind};
use crate::syntax::token::Pos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Number(f32),
    Vec2(Vec2),
    Colour(Colour),
}

pub struct Pixel {
    pub p: Vec2,
    pub uv: Vec2,
    pub time: f32,
    pub px: f32,
    pub size: Vec2,
}

pub fn run(program: &Program, pixel: &Pixel) -> Result<Colour, String> {
    let mut env = Env::new(pixel);
    for stmt in &program.body {
        env.statement(stmt)?;
    }
    env.out.ok_or("this file never sets OUT".into())
}

struct Env {
    vars: HashMap<String, Value>,
    out: Option<Colour>,
}

impl Env {
    fn new(pixel: &Pixel) -> Env {
        let vars = HashMap::from([
            ("P".to_string(), Value::Vec2(pixel.p)),
            ("UV".to_string(), Value::Vec2(pixel.uv)),
            ("TIME".to_string(), Value::Number(pixel.time)),
            ("PX".to_string(), Value::Number(pixel.px)),
            ("SIZE".to_string(), Value::Vec2(pixel.size)),
        ]);
        Env { vars, out: None }
    }

    fn statement(&mut self, stmt: &Stmt) -> Result<(), String> {
        match &stmt.kind {
            StmtKind::Input { name, default, .. } => {
                let value = self.expr(default)?;
                self.vars.insert(name.clone(), value);
            }
            StmtKind::Let { name, value } => {
                let value = self.expr(value)?;
                self.vars.insert(name.clone(), value);
            }
            StmtKind::Assign { name, value } if name == "OUT" => {
                let Value::Colour(colour) = self.expr(value)? else {
                    return Err(at("OUT must be set to a colour", stmt.pos));
                };
                self.out = Some(colour);
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

    fn expr(&self, expr: &Expr) -> Result<Value, String> {
        let value = match &expr.kind {
            ExprKind::Number(n) => Value::Number(*n),
            ExprKind::Colour(rgb) => Value::Colour(Colour::hex(*rgb)),
            ExprKind::Name(name) => match self.vars.get(name) {
                Some(value) => *value,
                None => return Err(at(&format!("unknown name `{name}`"), expr.pos)),
            },
            ExprKind::Negate(inner) => match self.expr(inner)? {
                Value::Number(n) => Value::Number(-n),
                Value::Vec2(v) => Value::Vec2(Vec2 { x: -v.x, y: -v.y }),
                Value::Colour(_) => return Err(at("can't negate a colour", expr.pos)),
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
                (Value::Vec2(v), "x") => Value::Number(v.x),
                (Value::Vec2(v), "y") => Value::Number(v.y),
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
        (Add, Number(a), Number(b)) => Number(a + b),
        (Sub, Number(a), Number(b)) => Number(a - b),
        (Mul, Number(a), Number(b)) => Number(a * b),
        (Div, Number(a), Number(b)) => Number(a / b),

        (Sub, Vec2(a), Vec2(b)) => Vec2(a - b),

        (Add, Colour(a), Colour(b)) => Colour(a + b),
        (Mul, Colour(c), Number(k)) | (Mul, Number(k), Colour(c)) => Colour(c * k),

        _ => return Err(format!("can't use {op:?} on {left:?} and {right:?}")),
    };
    Ok(result)
}

fn call(name: &str, args: &[Value]) -> Result<Value, String> {
    use Value::*;

    let result = match (name, args) {
        ("sin", [Number(x)]) => Number(x.sin()),

        ("circle", [Vec2(p), Number(r)]) => Number(pixel::circle(*p, *r)),

        ("fill", [Number(d), Colour(c)]) => Colour(pixel::fill(*d, *c)),
        ("glow", [Number(d), Colour(c), Number(w)]) => Colour(pixel::glow(*d, *c, *w)),

        _ => return Err(format!("unknown function `{name}`, or wrong arguments")),
    };
    Ok(result)
}

fn at(message: &str, pos: Pos) -> String {
    format!("{message} at line {}, column {}", pos.line, pos.col)
}
