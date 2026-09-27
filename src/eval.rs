use std::rc::Rc;
use std::sync::LazyLock;

use crate::pixel::{self, Rgba, Vec2};
use crate::shape::{self, Bounds};
use crate::syntax::ast::{BinOp, Expr, ExprKind, Program, Stmt, StmtKind};
use crate::syntax::lexer::lex;
use crate::syntax::parser::parse_prelude;
use crate::syntax::token::Pos;
use crate::tape::{Scalar, Tape};

static PRELUDE: LazyLock<Vec<Stmt>> = LazyLock::new(|| {
    let tokens = lex(include_str!("prelude.fld"), 1).expect("the prelude lexes");
    parse_prelude(tokens).expect("the prelude parses")
});

#[derive(Clone)]
pub enum Value<'a> {
    Num(Scalar),
    Vec2(Vec2),
    Rgba(Rgba),
    Func(Rc<Closure<'a>>),
    Shape(Rc<Shape<'a>>),
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

pub struct Shape<'a> {
    kind: ShapeKind<'a>,
    bounds: Bounds,
}

enum ShapeKind<'a> {
    Circle(Scalar),
    Rect(Vec2),
    Segment(Vec2, Vec2),
    Custom(Rc<Closure<'a>>),
}

impl Value<'_> {
    fn type_name(&self) -> &'static str {
        match self {
            Value::Num(_) => "num",
            Value::Vec2(_) => "vec2",
            Value::Rgba(_) => "rgba",
            Value::Func(_) => "func",
            Value::Shape(_) => "shape",
        }
    }

    fn a_type(&self) -> String {
        let article = if matches!(self, Value::Rgba(_)) {
            "an"
        } else {
            "a"
        };
        format!("{article} {}", self.type_name())
    }
}

#[derive(Clone, Copy)]
pub struct Pixel {
    pub pos: Vec2,
    pub time: Scalar,
    pub px: Scalar,
    pub size: Vec2,
}

pub struct Prelude {
    vars: Vec<(&'static str, Value<'static>)>,
}

impl Prelude {
    pub fn load(size: Vec2) -> Result<Prelude, String> {
        let origin = pixel::vec2(0.0, 0.0);
        let mut env = Env {
            pixel: Pixel {
                pos: origin,
                time: Scalar::from(0.0),
                px: Scalar::from(1.0),
                size,
            },
            globals: &[],
            vars: Vec::new(),
            canvas: Rgba::CLEAR,
        };
        for stmt in PRELUDE.iter() {
            env.statement(stmt)
                .map_err(|e| format!("in the prelude: {e}"))?;
        }
        Ok(Prelude { vars: env.vars })
    }
}

pub fn compile(program: &Program) -> Result<Tape, String> {
    let size = pixel::vec2(program.header.width as f32, program.header.height as f32);
    let prelude = Prelude::load(size)?;
    Tape::record(4, |inputs| {
        let pixel = Pixel {
            pos: pixel::vec2(inputs[0], inputs[1]),
            time: inputs[2],
            px: inputs[3],
            size,
        };
        Ok(run(program, &prelude, pixel)?.channels())
    })
}

pub fn run(program: &Program, prelude: &Prelude, pixel: Pixel) -> Result<Rgba, String> {
    let mut env = Env {
        pixel,
        globals: &prelude.vars,
        vars: Vec::new(),
        canvas: Rgba::CLEAR,
    };
    for stmt in &program.body {
        env.statement(stmt)?;
    }
    Ok(env.canvas)
}

struct Env<'a> {
    pixel: Pixel,
    globals: &'a [(&'a str, Value<'a>)],
    vars: Vec<(&'a str, Value<'a>)>,
    canvas: Rgba,
}

impl<'a> Env<'a> {
    fn statement(&mut self, stmt: &'a Stmt) -> Result<(), String> {
        match &stmt.kind {
            StmtKind::Input {
                name,
                range,
                default,
            } => {
                let value = self.expr(default)?;
                self.check_input(range, &value, stmt.pos)?;
                self.vars.push((name, value));
            }
            StmtKind::Let { name, value } => {
                let value = self.expr(value)?;
                self.vars.push((name, value));
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
            StmtKind::Draw(value) => match self.expr(value)? {
                Value::Rgba(col) => self.canvas = col.valid().over(self.canvas),
                other => {
                    let message = format!("`draw` needs an rgba, not {}", other.a_type());
                    return Err(at(&message, stmt.pos));
                }
            },
        }
        Ok(())
    }

    fn check_input(
        &self,
        range: &'a Option<(Expr, Expr)>,
        default: &Value<'a>,
        pos: Pos,
    ) -> Result<(), String> {
        let message = match (range, default) {
            (None, Value::Rgba(_)) => return Ok(()),
            (Some((min, max)), Value::Num(n)) => match (self.expr(min)?, self.expr(max)?) {
                (Value::Num(lo), Value::Num(hi)) => match (lo.known(), hi.known(), n.known()) {
                    (Some(lo), Some(hi), Some(n)) if lo <= n && n <= hi => return Ok(()),
                    (Some(lo), Some(hi), Some(n)) => {
                        format!("the default {n} is outside {lo}..{hi}")
                    }
                    _ => "an input's range and default must be constants".into(),
                },
                _ => "an input's range must be two nums".into(),
            },
            (Some(_), other) => format!(
                "an input with a range must default to a num, not {}",
                other.a_type()
            ),
            (None, other) => format!(
                "an input without a range must default to an rgba, not {}",
                other.a_type()
            ),
        };
        Err(at(&message, pos))
    }

    fn lookup(&self, name: &str) -> Option<Value<'a>> {
        let builtin = match name {
            "POS" => Value::Vec2(self.pixel.pos),
            "TIME" => Value::Num(self.pixel.time),
            "PX" => Value::Num(self.pixel.px),
            "SIZE" => Value::Vec2(self.pixel.size),
            _ => {
                return self
                    .vars
                    .iter()
                    .rev()
                    .chain(self.globals.iter().rev())
                    .find(|(var, _)| *var == name)
                    .map(|(_, value)| value.clone());
            }
        };
        Some(builtin)
    }

    fn expr(&self, expr: &'a Expr) -> Result<Value<'a>, String> {
        let value = match &expr.kind {
            ExprKind::Num(n) => Value::Num(Scalar::from(*n)),
            ExprKind::Rgba(hex) => Value::Rgba(Rgba::hex(*hex)),
            ExprKind::Name(name) => match self.lookup(name) {
                Some(value) => value,
                None => return Err(at(&format!("unknown name `{name}`"), expr.pos)),
            },
            ExprKind::Negate(inner) => match self.expr(inner)? {
                Value::Num(n) => Value::Num(-n),
                Value::Vec2(v) => Value::Vec2(Vec2 { x: -v.x, y: -v.y }),
                other => {
                    let message = format!("can't negate {}", other.a_type());
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
                        let message = format!("`{name}` is {}, not a func", other.a_type());
                        return Err(at(&message, expr.pos));
                    }
                    None => self.native(name, &args, expr.pos)?,
                }
            }
            ExprKind::Field { target, field } => {
                let value = self.expr(target)?;
                match swizzle(&value, field) {
                    Some(value) => value,
                    None => {
                        let message = format!("{} has no field `{field}`", value.a_type());
                        return Err(at(&message, expr.pos));
                    }
                }
            }
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
            globals: self.globals,
            vars: closure.captured.clone(),
            canvas: Rgba::CLEAR,
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

    fn native(&self, name: &str, args: &[Value<'a>], pos: Pos) -> Result<Value<'a>, String> {
        use Value::*;

        let result = match (name, args) {
            ("sin", [Num(x)]) => Num(x.sin()),
            ("cos", [Num(x)]) => Num(x.cos()),
            ("atan2", [Num(y), Num(x)]) => Num(y.atan2(*x)),
            ("sqrt", [Num(x)]) => Num(x.sqrt()),
            ("exp", [Num(x)]) => Num(x.exp()),
            ("pow", [Num(x), Num(by)]) => Num(x.powf(*by)),
            ("floor", [Num(x)]) => Num(x.floor()),
            ("abs", [Num(x)]) => Num(x.abs()),
            ("min", [Num(a), Num(b)]) => Num(a.min(*b)),
            ("max", [Num(a), Num(b)]) => Num(a.max(*b)),

            ("vec2", [Num(x), Num(y)]) => Vec2(pixel::vec2(*x, *y)),

            ("rgba", [Num(r), Num(g), Num(b), Num(a)]) => {
                Rgba(pixel::Rgba::from_srgb(*r, *g, *b, *a))
            }

            ("hash", [Vec2(pt)]) => Num(pixel::hash(*pt)),

            ("circle", [Num(r)]) => {
                new_shape(ShapeKind::Circle(*r), Bounds::around(pixel::vec2(*r, *r)))
            }
            ("rect", [Num(w), Num(h)]) => {
                let half = pixel::vec2(*w / 2.0, *h / 2.0);
                new_shape(ShapeKind::Rect(half), Bounds::around(half))
            }
            ("segment", [Vec2(from), Vec2(to)]) => {
                new_shape(ShapeKind::Segment(*from, *to), Bounds::between(*from, *to))
            }
            ("shape", [Func(fn_)]) => new_shape(
                ShapeKind::Custom(fn_.clone()),
                Bounds::around(self.pixel.size * 0.5),
            ),
            ("dist", [Shape(sh), Vec2(pt)]) => Num(self.dist(sh, *pt, pos)?),
            ("anchor", [Shape(sh), Vec2(anc)]) => Vec2(sh.bounds.anchor(*anc)),

            _ => {
                let types: Vec<&str> = args.iter().map(Value::type_name).collect();
                let message = format!(
                    "unknown function `{name}`, or it can't take ({})",
                    types.join(", ")
                );
                return Err(at(&message, pos));
            }
        };
        Ok(result)
    }

    fn dist(&self, sh: &Shape<'a>, pt: Vec2, pos: Pos) -> Result<Scalar, String> {
        let d = match &sh.kind {
            ShapeKind::Circle(r) => shape::circle(pt, *r),
            ShapeKind::Rect(half) => shape::rect(pt, *half),
            ShapeKind::Segment(from, to) => shape::segment(pt, *from, *to),
            ShapeKind::Custom(fn_) => match self.apply(fn_, vec![Value::Vec2(pt)], pos)? {
                Value::Num(d) => d,
                other => {
                    let message = format!(
                        "a shape's function must return a num, not {}",
                        other.a_type()
                    );
                    return Err(at(&message, pos));
                }
            },
        };
        Ok(d)
    }
}

fn new_shape<'a>(kind: ShapeKind<'a>, bounds: Bounds) -> Value<'a> {
    Value::Shape(Rc::new(Shape { kind, bounds }))
}

fn binary<'a>(op: BinOp, left: Value<'a>, right: Value<'a>) -> Result<Value<'a>, String> {
    use BinOp::*;
    use Value::*;

    let result = match (op, left, right) {
        (Add, Num(a), Num(b)) => Num(a + b),
        (Sub, Num(a), Num(b)) => Num(a - b),
        (Mul, Num(a), Num(b)) => Num(a * b),
        (Div, Num(a), Num(b)) => Num(a / b),

        (Add, Vec2(a), Vec2(b)) => Vec2(a + b),
        (Sub, Vec2(a), Vec2(b)) => Vec2(a - b),
        (Mul, Vec2(a), Vec2(b)) => Vec2(a.times(b)),
        (Div, Vec2(a), Vec2(b)) => Vec2(a.per(b)),

        (Mul, Vec2(v), Num(k)) | (Mul, Num(k), Vec2(v)) => Vec2(v * k),
        (Div, Vec2(v), Num(k)) => Vec2(v / k),

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

fn swizzle<'a>(value: &Value<'a>, field: &str) -> Option<Value<'a>> {
    match value {
        Value::Vec2(v) => {
            let parts = pick(field, |c| match c {
                'x' => Some(v.x),
                'y' => Some(v.y),
                _ => None,
            })?;
            match parts[..] {
                [n] => Some(Value::Num(n)),
                [x, y] => Some(Value::Vec2(pixel::vec2(x, y))),
                _ => None,
            }
        }
        Value::Rgba(col) => {
            let [r, g, b, a] = col.to_srgb();
            let parts = pick(field, |c| match c {
                'r' => Some(r),
                'g' => Some(g),
                'b' => Some(b),
                'a' => Some(a),
                _ => None,
            })?;
            match parts[..] {
                [n] => Some(Value::Num(n)),
                [r, g, b] => Some(Value::Rgba(Rgba::from_srgb(r, g, b, Scalar::from(1.0)))),
                [r, g, b, a] => Some(Value::Rgba(Rgba::from_srgb(r, g, b, a))),
                _ => None,
            }
        }
        _ => None,
    }
}

fn pick(field: &str, part: impl Fn(char) -> Option<Scalar>) -> Option<Vec<Scalar>> {
    field.chars().map(part).collect()
}

fn symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
    }
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
        let size = pixel::vec2(256.0, 256.0);
        let prelude = Prelude::load(size)?;
        let mut env = Env {
            pixel: Pixel {
                pos: pixel::vec2(0.0, 0.0),
                time: Scalar::from(0.0),
                px: Scalar::from(1.0),
                size,
            },
            globals: &prelude.vars,
            vars: Vec::new(),
            canvas: Rgba::CLEAR,
        };
        for stmt in &program.body {
            env.statement(stmt)?;
        }
        match env.lookup("result") {
            Some(Value::Num(n)) => n.known().ok_or("`result` is not known".into()),
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

    #[test]
    fn circle_distance() {
        assert_eq!(num("let result = dist(circle(10), vec2(30, 40))"), Ok(40.0));
    }

    #[test]
    fn rect_distance_outside_and_inside() {
        assert_eq!(num("let result = dist(rect(20, 10), vec2(15, 0))"), Ok(5.0));
        assert_eq!(num("let result = dist(rect(20, 10), vec2(0, 0))"), Ok(-5.0));
    }

    #[test]
    fn segment_distance() {
        let line = "let line = segment(vec2(-10, 0), vec2(10, 0))\n";
        assert_eq!(
            num(&format!("{line}let result = dist(line, vec2(0, 7))")),
            Ok(7.0)
        );
        assert_eq!(
            num(&format!("{line}let result = dist(line, vec2(13, 4))")),
            Ok(5.0)
        );
    }

    #[test]
    fn custom_shape_distance() {
        let src = "let ring = shape(pt => dist(circle(10), pt) - 2)\nlet result = dist(ring, vec2(20, 0))";
        assert_eq!(num(src), Ok(8.0));
    }

    #[test]
    fn shape_moved_by_fold() {
        let src = "let moved = shape(pt => dist(circle(10), pt - vec2(100, 0)))\nlet result = dist(moved, vec2(100, 0))";
        assert_eq!(num(src), Ok(-10.0));
    }

    #[test]
    fn anchor_reads_the_box() {
        assert_eq!(
            num("let result = anchor(rect(20, 10), vec2(1, 1)).x"),
            Ok(10.0)
        );
        assert_eq!(
            num("let result = anchor(rect(20, 10), vec2(0, -1)).y"),
            Ok(-5.0)
        );
    }

    #[test]
    fn custom_shape_must_return_num() {
        let error =
            num("let bad = shape(pt => pt)\nlet result = dist(bad, vec2(0, 0))").unwrap_err();
        assert!(error.contains("must return a num, not a vec2"), "{error}");
    }

    #[test]
    fn shapes_are_not_numbers() {
        let error = num("let result = circle(10) + 1").unwrap_err();
        assert!(error.contains("can't use `+` on shape and num"), "{error}");
    }

    #[test]
    fn maths_natives() {
        assert_eq!(num("let result = cos(0)"), Ok(1.0));
        assert_eq!(num("let result = atan2(0, 1)"), Ok(0.0));
        assert_eq!(num("let result = sqrt(16)"), Ok(4.0));
        assert_eq!(num("let result = exp(0)"), Ok(1.0));
        assert_eq!(num("let result = pow(2, 10)"), Ok(1024.0));
        assert_eq!(num("let result = floor(-1.5)"), Ok(-2.0));
        assert_eq!(num("let result = abs(-3)"), Ok(3.0));
        assert_eq!(num("let result = min(2, 5)"), Ok(2.0));
        assert_eq!(num("let result = max(2, 5)"), Ok(5.0));
    }

    #[test]
    fn vec2_arithmetic() {
        assert_eq!(num("let result = (vec2(1, 2) + vec2(3, 4)).y"), Ok(6.0));
        assert_eq!(num("let result = (vec2(1, 2) * vec2(3, 4)).y"), Ok(8.0));
        assert_eq!(num("let result = (vec2(6, 8) / vec2(3, 4)).x"), Ok(2.0));
        assert_eq!(num("let result = (2 * vec2(1, 2)).y"), Ok(4.0));
        assert_eq!(num("let result = (vec2(1, 2) * 2).y"), Ok(4.0));
        assert_eq!(num("let result = (vec2(6, 8) / 2).y"), Ok(4.0));
    }

    #[test]
    fn arithmetic_outside_the_table_is_an_error() {
        let error = num("let result = 1 / vec2(1, 2)").unwrap_err();
        assert!(error.contains("can't use `/` on num and vec2"), "{error}");
        let error = num("let result = vec2(1, 2) + 1").unwrap_err();
        assert!(error.contains("can't use `+` on vec2 and num"), "{error}");
    }

    #[test]
    fn hash_is_repeatable_and_in_range() {
        let first = num("let result = hash(vec2(3, 7))").unwrap();
        assert_eq!(num("let result = hash(vec2(3, 7))"), Ok(first));
        assert!((0.0..1.0).contains(&first));
        assert_ne!(num("let result = hash(vec2(7, 3))"), Ok(first));
    }

    #[test]
    fn hash_treats_negative_zero_as_zero() {
        assert_eq!(
            num("let result = hash(vec2(-0, 0))"),
            num("let result = hash(vec2(0, 0))")
        );
    }

    #[test]
    fn wrong_arguments_name_the_types() {
        let error = num("let result = sqrt(vec2(1, 2))").unwrap_err();
        assert!(error.contains("can't take (vec2)"), "{error}");
    }

    fn close(body: &str, want: f32) {
        let got = num(body).unwrap();
        assert!((got - want).abs() < 1e-3, "{body}: got {got}, want {want}");
    }

    #[test]
    fn prelude_maths() {
        close("let result = clamp(5, 0, 1)", 1.0);
        close("let result = mix(2, 4, 0.5)", 3.0);
        close("let result = smoothstep(0, 1, 0.5)", 0.5);
        close("let result = length(vec2(3, 4))", 5.0);
        close("let result = DEG * 360", 6.283_185);
    }

    #[test]
    fn prelude_moves_shapes() {
        close(
            "let result = dist(circle(10) |> at(100, 0), vec2(100, 0))",
            -10.0,
        );
        close(
            "let result = dist(circle(10) |> at(50, 0) |> mirror, vec2(-50, 0))",
            -10.0,
        );
        close(
            "let result = dist(circle(10) |> at(100, 0) |> spin(90 * DEG), vec2(0, 100))",
            -10.0,
        );
        close(
            "let result = dist(circle(10) |> pin(LEFT, vec2(0, 0)), vec2(10, 0))",
            -10.0,
        );
    }

    #[test]
    fn prelude_combines_shapes() {
        close(
            "let result = dist(union(circle(10), circle(10) |> at(100, 0)), vec2(100, 0))",
            -10.0,
        );
        close(
            "let result = dist(circle(10) |> outline(2), vec2(10, 0))",
            -1.0,
        );
        close(
            "let result = dist(rounded_rect(40, 20, 5), vec2(0, 0))",
            -10.0,
        );
    }

    #[test]
    fn prelude_frame_and_cover() {
        close("let result = anchor(FRAME, TOP_RIGHT).x", 128.0);
        close("let result = cover(circle(10))", 1.0);
        close("let result = cover(circle(10) |> at(100, 0))", 0.0);
    }

    #[test]
    fn prelude_noise_in_range() {
        let n = num("let result = noise(vec2(1.5, 2.5))").unwrap();
        assert!((0.0..1.0).contains(&n), "{n}");
    }

    #[test]
    fn prelude_keeps_its_own_names() {
        let src = "func clamp(x, lo, hi) { return 99 }\n";
        close(&format!("{src}let result = smoothstep(0, 1, 0.5)"), 0.5);
        close(&format!("{src}let result = clamp(0, 0, 1)"), 99.0);
    }

    #[test]
    fn colours_read_back_as_written() {
        close("let result = #ff8800.r", 1.0);
        close("let result = #ff8800.g", 136.0 / 255.0);
        close("let result = #ff8800.a", 1.0);
        close("let result = #00000066.a", 0.4);
    }

    #[test]
    fn multiplying_a_colour_fades_it() {
        close("let result = (#ff8800 * 0.5).a", 0.5);
        close("let result = (#ff8800 * 0.5).r", 1.0);
    }

    #[test]
    fn rgba_makes_a_colour() {
        close("let result = rgba(1, 0.5, 0, 0.25).g", 0.5);
        close("let result = rgba(1, 0.5, 0, 0.25).a", 0.25);
    }

    #[test]
    fn vec2_swizzles() {
        close("let result = vec2(1, 2).yx.x", 2.0);
        close("let result = vec2(1, 2).xx.y", 1.0);
        let error = num("let result = vec2(1, 2).z").unwrap_err();
        assert!(error.contains("a vec2 has no field `z`"), "{error}");
        let error = num("let result = vec2(1, 2).xyx").unwrap_err();
        assert!(error.contains("no field `xyx`"), "{error}");
    }

    #[test]
    fn rgba_swizzles() {
        close("let result = #ff880066.rgb.a", 1.0);
        close("let result = #ff8800.bgr.r", 0.0);
        close("let result = #ff880066.bgra.a", 0.4);
        let error = num("let result = #ff8800.rg").unwrap_err();
        assert!(error.contains("an rgba has no field `rg`"), "{error}");
    }

    #[test]
    fn shade_darkens_and_lightens() {
        close("let result = shade(#808080, 1).r", 1.0);
        close("let result = shade(#808080, -1).r", 0.0);
        close("let result = shade(#80808080, 1).a", 128.0 / 255.0);
    }

    fn canvas_at(src: &str, pos: Vec2) -> Result<[f32; 4], String> {
        let (header, rest) = parse_header(src)?;
        let program = parse(header, lex(rest, 2)?)?;
        let size = pixel::vec2(program.header.width as f32, program.header.height as f32);
        let prelude = Prelude::load(size)?;
        let pixel = Pixel {
            pos,
            time: Scalar::from(0.0),
            px: Scalar::from(1.0),
            size,
        };
        let canvas = run(&program, &prelude, pixel)?;
        Ok(canvas.channels().map(|c| c.known().unwrap()))
    }

    fn hex(rgba: u32) -> [f32; 4] {
        Rgba::hex(rgba).channels().map(|c| c.known().unwrap())
    }

    fn byte_colour([r, g, b, _]: [f32; 4]) -> u32 {
        pixel::to_u32(r, g, b)
    }

    fn canvas(body: &str) -> Result<[f32; 4], String> {
        canvas_at(&format!("~fold v1 256x256\n{body}"), pixel::vec2(0.0, 0.0))
    }

    #[test]
    fn canvas_starts_transparent() {
        assert_eq!(canvas("let x = 1"), Ok([0.0; 4]));
    }

    #[test]
    fn opaque_draw_covers_what_is_below() {
        let top = canvas("draw #0000ff\ndraw #ff0000").unwrap();
        assert_eq!(top, hex(0xff0000ff));
    }

    #[test]
    fn transparent_draw_blends_in_linear_light() {
        let mixed = canvas("draw #0000ff\ndraw #ff0000 * 0.5").unwrap();
        assert_eq!(mixed[3], 1.0);
        assert!((mixed[0] - 0.5).abs() < 1e-6, "{mixed:?}");
        assert!((mixed[2] - 0.5).abs() < 1e-6, "{mixed:?}");
    }

    #[test]
    fn draw_makes_colours_valid() {
        let bright = canvas("draw #ffffff * 3").unwrap();
        assert_eq!(bright, hex(0xffffffff));
        let summed = canvas("draw #ff0000 * 0.5 + #00ff00 * 0.5 + #0000ff").unwrap();
        assert_eq!(summed[3], 1.0);
        assert!(summed[0] <= summed[3] && summed[2] <= summed[3]);
    }

    #[test]
    fn draw_needs_a_colour() {
        let error = canvas("draw circle(10)").unwrap_err();
        assert!(
            error.contains("`draw` needs an rgba, not a shape"),
            "{error}"
        );
    }

    #[test]
    fn old_names_are_gone() {
        assert!(
            canvas("OUT = #ffffff")
                .unwrap_err()
                .contains("expected a statement")
        );
        assert!(
            num("let result = P.x")
                .unwrap_err()
                .contains("unknown name `P`")
        );
        assert!(
            num("let result = UV.x")
                .unwrap_err()
                .contains("unknown name `UV`")
        );
    }

    #[test]
    fn sun_example_renders() {
        let sun = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/sun.fld"));
        let centre = canvas_at(sun, pixel::vec2(0.0, 0.0)).unwrap();
        assert_eq!(byte_colour(centre), 0xffaa00);
        let corner = canvas_at(sun, pixel::vec2(-120.0, -120.0)).unwrap();
        assert_eq!(corner[3], 1.0);
        assert_ne!(byte_colour(corner), 0xffaa00);
    }

    #[test]
    fn cookbook_recipes_render() {
        let book = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/COOKBOOK.md"));
        let recipes = book.split("```").skip(1).step_by(2);
        for recipe in recipes {
            let src = format!("~fold v1 256x256\n{}", recipe.trim_start());
            for pos in [
                pixel::vec2(0.0, 0.0),
                pixel::vec2(40.0, -30.0),
                pixel::vec2(-120.0, 110.0),
            ] {
                if let Err(e) = canvas_at(&src, pos) {
                    panic!("{e}\n{recipe}");
                }
            }
        }
    }

    #[test]
    fn inputs_follow_the_spec() {
        assert_eq!(num("input result: 0..10 = 4"), Ok(4.0));
        assert!(canvas("input accent = #3366ff\ndraw accent").is_ok());
        let error = canvas("input x: 0..1 = 2").unwrap_err();
        assert!(error.contains("the default 2 is outside 0..1"), "{error}");
        let error = canvas("input x = 5").unwrap_err();
        assert!(
            error.contains("without a range must default to an rgba, not a num"),
            "{error}"
        );
        let error = canvas("input x: 0..1 = #ffffff").unwrap_err();
        assert!(
            error.contains("with a range must default to a num, not an rgba"),
            "{error}"
        );
    }

    #[test]
    fn nan_colours_draw_as_transparent() {
        let red = canvas("draw #ff0000\ndraw #ffffff * (0 / 0)").unwrap();
        assert_eq!(red, hex(0xff0000ff));
    }

    #[test]
    fn capital_names_belong_to_the_engine() {
        let error = canvas("let TAU = 3").unwrap_err();
        assert!(error.contains("`TAU` is all capitals"), "{error}");
        let error = canvas("func f(POS) { return 1 }").unwrap_err();
        assert!(error.contains("`POS` is all capitals"), "{error}");
        assert!(canvas("let Sun = 1").is_ok());
    }

    #[test]
    fn names_are_defined_once() {
        let error = canvas("let a = 1\nlet a = 2").unwrap_err();
        assert!(error.contains("`a` is already defined"), "{error}");
        let error = canvas("func f(x, x) { return x }").unwrap_err();
        assert!(error.contains("`x` is already a parameter"), "{error}");
        let error = canvas("func f(x) { let x = 1\nreturn x }").unwrap_err();
        assert!(error.contains("`x` is already defined"), "{error}");
        assert!(canvas("func glow(sh, col, amt) { return col }").is_ok());
    }

    #[test]
    fn other_versions_are_refused() {
        let error = canvas_at("~fold v0 256x256\n", pixel::vec2(0.0, 0.0)).unwrap_err();
        assert!(error.contains("this engine supports v1"), "{error}");
        let error = canvas_at("~fold v2 256x256\n", pixel::vec2(0.0, 0.0)).unwrap_err();
        assert!(error.contains("this file is Fold v2"), "{error}");
    }

    fn same_on_tape(src: &str) {
        let (header, rest) = parse_header(src).unwrap();
        let program = parse(header, lex(rest, 2).unwrap()).unwrap();
        let tape = compile(&program).unwrap();
        let size = pixel::vec2(program.header.width as f32, program.header.height as f32);
        let prelude = Prelude::load(size).unwrap();
        let mut slots = tape.slots();
        for time in [0.0, 1.3] {
            for y in (-128..128).step_by(9) {
                for x in (-128..128).step_by(9) {
                    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
                    let pixel = Pixel {
                        pos: pixel::vec2(x, y),
                        time: Scalar::from(time),
                        px: Scalar::from(1.0),
                        size,
                    };
                    let want = run(&program, &prelude, pixel).unwrap().channels();
                    let got = tape.run(&mut slots, &[x, y, time, 1.0]);
                    for (want, got) in want.iter().zip(got) {
                        let want = want.known().unwrap();
                        assert_eq!(want.to_bits(), got.to_bits(), "at ({x}, {y}): {src}");
                    }
                }
            }
        }
    }

    #[test]
    fn tape_matches_the_reference_bit_for_bit() {
        same_on_tape(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/sun.fld"
        )));
        let book = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/COOKBOOK.md"));
        for recipe in book.split("```").skip(1).step_by(2) {
            same_on_tape(&format!("~fold v1 256x256\n{}", recipe.trim_start()));
        }
    }

    #[test]
    fn errors_are_found_when_compiling() {
        let src = "~fold v1 256x256\nlet sun = circle(10)\ndraw sun |> fill(#ffaa00) + 1";
        let (header, rest) = parse_header(src).unwrap();
        let program = parse(header, lex(rest, 2).unwrap()).unwrap();
        let error = compile(&program).unwrap_err();
        assert!(error.contains("can't use `+` on rgba and num"), "{error}");
    }

    #[test]
    fn constant_work_leaves_the_tape() {
        let src = "~fold v1 256x256\nlet k = sin(1) * 20 + 3\ndraw #ffffff * k";
        let (header, rest) = parse_header(src).unwrap();
        let program = parse(header, lex(rest, 2).unwrap()).unwrap();
        let tape = compile(&program).unwrap();
        assert!(
            tape.steps
                .iter()
                .all(|step| step.op != crate::tape::Op::Sin)
        );
    }
}
