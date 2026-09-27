use std::rc::Rc;

use crate::pixel::{self, Rgba, Vec2};
use crate::syntax::ast::{BinOp, Expr, ExprKind, Program, Stmt, StmtKind};
use crate::syntax::token::Pos;

#[derive(Clone)]
pub enum Value<'a> {
    Num(f32),
    Vec2(Vec2),
    Rgba(Rgba),
    Func(Rc<Closure<'a>>),
}

pub struct Closure<'a> {
    params: Vec<&'a str>,
    body: Body<'a>,
    captured: Vec<(&'a str, Value<'a>)>,
}

enum Body<'a> {
    Block(&'a [Stmt]),
    Expr(&'a Expr),
}

impl Value<'_> {
    fn type_name(&self) -> &'static str {
        match self {
            Value::Num(_) => "num",
            Value::Vec2(_) => "vec2",
            Value::Rgba(_) => "rgba",
            Value::Func(_) => "func",
        }
    }
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
    vars: Vec<(&'a str, Value<'a>)>,
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
            StmtKind::Func { name, params, body } => {
                let closure = Closure {
                    params: params.iter().map(String::as_str).collect(),
                    body: Body::Block(body),
                    captured: self.vars.clone(),
                };
                self.vars.push((name, Value::Func(Rc::new(closure))));
            }
            StmtKind::Return(_) => {
                return Err(at("`return` belongs inside a `func`", stmt.pos));
            }
            StmtKind::Draw(_) => {
                return Err(at("`draw` isn't supported yet", stmt.pos));
            }
        }
        Ok(())
    }

    fn lookup(&self, name: &str) -> Option<Value<'a>> {
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
                    .map(|(_, value)| value.clone());
            }
        };
        Some(builtin)
    }

    fn expr(&self, expr: &'a Expr) -> Result<Value<'a>, String> {
        let value = match &expr.kind {
            ExprKind::Num(n) => Value::Num(*n),
            ExprKind::Rgba(hex) => Value::Rgba(Rgba::hex(*hex >> 8)),
            ExprKind::Name(name) => match self.lookup(name) {
                Some(value) => value,
                None => return Err(at(&format!("unknown name `{name}`"), expr.pos)),
            },
            ExprKind::Negate(inner) => match self.expr(inner)? {
                Value::Num(n) => Value::Num(-n),
                Value::Vec2(v) => Value::Vec2(Vec2 { x: -v.x, y: -v.y }),
                other => {
                    let message = format!("can't negate a {}", other.type_name());
                    return Err(at(&message, expr.pos));
                }
            },
            ExprKind::Binary { op, left, right } => {
                binary(*op, self.expr(left)?, self.expr(right)?).map_err(|e| at(&e, expr.pos))?
            }
            ExprKind::Call { name, args } => {
                let args = args
                    .iter()
                    .map(|arg| self.expr(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                match self.lookup(name) {
                    Some(Value::Func(closure)) => self.apply(&closure, args, expr.pos)?,
                    Some(other) => {
                        let message = format!("`{name}` is a {}, not a func", other.type_name());
                        return Err(at(&message, expr.pos));
                    }
                    None => call(name, &args).map_err(|e| at(&e, expr.pos))?,
                }
            }
            ExprKind::Field { target, field } => match (self.expr(target)?, field.as_str()) {
                (Value::Vec2(v), "x") => Value::Num(v.x),
                (Value::Vec2(v), "y") => Value::Num(v.y),
                _ => return Err(at(&format!("no field `{field}`"), expr.pos)),
            },
            ExprKind::Lambda { param, body } => Value::Func(Rc::new(Closure {
                params: vec![param.as_str()],
                body: Body::Expr(body),
                captured: self.vars.clone(),
            })),
        };
        Ok(value)
    }

    fn apply(
        &self,
        closure: &Closure<'a>,
        args: Vec<Value<'a>>,
        pos: Pos,
    ) -> Result<Value<'a>, String> {
        if args.len() != closure.params.len() {
            let message = format!(
                "expected {} arguments, found {}",
                closure.params.len(),
                args.len()
            );
            return Err(at(&message, pos));
        }

        let mut env = Env {
            pixel: self.pixel,
            vars: closure.captured.clone(),
            out: None,
        };
        env.vars.extend(closure.params.iter().copied().zip(args));

        match closure.body {
            Body::Block(body) => env.block(body, pos),
            Body::Expr(body) => env.expr(body),
        }
    }

    fn block(&mut self, body: &'a [Stmt], pos: Pos) -> Result<Value<'a>, String> {
        for stmt in body {
            match &stmt.kind {
                StmtKind::Let { name, value } => {
                    let value = self.expr(value)?;
                    self.vars.push((name, value));
                }
                StmtKind::Return(value) => return self.expr(value),
                _ => {
                    return Err(at("a `func` may only contain `let` and `return`", stmt.pos));
                }
            }
        }
        Err(at("this func ended without `return`", pos))
    }
}

fn binary<'a>(op: BinOp, left: Value<'a>, right: Value<'a>) -> Result<Value<'a>, String> {
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

        (op, left, right) => {
            return Err(format!(
                "can't use `{}` on {} and {}",
                symbol(op),
                left.type_name(),
                right.type_name()
            ));
        }
    };
    Ok(result)
}

fn symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
    }
}

fn call<'a>(name: &str, args: &[Value<'a>]) -> Result<Value<'a>, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::header::parse_header;
    use crate::syntax::lexer::lex;
    use crate::syntax::parser::parse;

    fn num(body: &str) -> Result<f32, String> {
        let src = format!("~fold v1 256x256\n{body}");
        let (header, rest) = parse_header(&src)?;
        let program = parse(header, lex(rest, 2)?)?;
        let pixel = Pixel {
            p: Vec2 { x: 0.0, y: 0.0 },
            uv: Vec2 { x: 0.0, y: 0.0 },
            time: 0.0,
            px: 1.0,
            size: Vec2 { x: 256.0, y: 256.0 },
        };
        let mut env = Env {
            pixel: &pixel,
            vars: Vec::new(),
            out: None,
        };
        for stmt in &program.body {
            env.statement(stmt)?;
        }
        match env.lookup("result") {
            Some(Value::Num(n)) => Ok(n),
            _ => Err("`result` is not a num".into()),
        }
    }

    #[test]
    fn func_returns() {
        assert_eq!(
            num("func double(x) { return x * 2 }\nlet result = double(21)"),
            Ok(42.0)
        );
    }

    #[test]
    fn func_body_lets() {
        let src =
            "func area(w, h) {\n    let a = w * h\n    return a / 2\n}\nlet result = area(4, 5)";
        assert_eq!(num(src), Ok(10.0));
    }

    #[test]
    fn func_sees_names_above_it() {
        let src = "let k = 3\nfunc times_k(x) { return x * k }\nlet result = times_k(5)";
        assert_eq!(num(src), Ok(15.0));
    }

    #[test]
    fn func_called_through_pipe() {
        let src = "func add(x, by) { return x + by }\nlet result = 1 |> add(2) |> add(3)";
        assert_eq!(num(src), Ok(6.0));
    }

    #[test]
    fn func_replaces_native() {
        let src = "func sin(x) { return 7 }\nlet result = sin(0)";
        assert_eq!(num(src), Ok(7.0));
    }

    #[test]
    fn func_cannot_recurse() {
        let error = num("func again(x) { return again(x) }\nlet result = again(1)").unwrap_err();
        assert!(error.contains("unknown function `again`"), "{error}");
    }

    #[test]
    fn func_wrong_argument_count() {
        let error = num("func double(x) { return x * 2 }\nlet result = double(1, 2)").unwrap_err();
        assert!(error.contains("expected 1 arguments, found 2"), "{error}");
    }

    #[test]
    fn func_without_return() {
        let error = num("func nothing(x) { let y = x }\nlet result = nothing(1)").unwrap_err();
        assert!(error.contains("without `return`"), "{error}");
    }

    #[test]
    fn inline_function_called() {
        assert_eq!(
            num("let double = x => x * 2\nlet result = double(21)"),
            Ok(42.0)
        );
    }

    #[test]
    fn inline_function_captures() {
        let src = "let k = 3\nlet times_k = x => x * k\nlet result = times_k(5)";
        assert_eq!(num(src), Ok(15.0));
    }

    #[test]
    fn inline_function_as_argument() {
        let src = "func twice(fn, x) { return fn(fn(x)) }\nlet result = twice(x => x + 1, 5)";
        assert_eq!(num(src), Ok(7.0));
    }

    #[test]
    fn inline_function_outlives_its_func() {
        let src =
            "func adder(by) { return x => x + by }\nlet add3 = adder(3)\nlet result = add3(4)";
        assert_eq!(num(src), Ok(7.0));
    }

    #[test]
    fn calling_a_num_is_an_error() {
        let error = num("let r = 5\nlet result = r(2)").unwrap_err();
        assert!(error.contains("`r` is a num, not a func"), "{error}");
    }
}
