mod native;
mod value;

use std::rc::Rc;
use std::sync::LazyLock;

use crate::maths::shape::Reach;
use crate::maths::{self, Rgba, Vec2, shape};
use crate::syntax::ast::{Expr, ExprKind, Program, Stmt, StmtKind};
use crate::syntax::lexer::lex;
use crate::syntax::parser::parse_prelude;
use crate::syntax::token::Pos;
use crate::tape::{self, INPUTS, Range, Scalar, TIME, Tape, X, Y};
use value::{Body, Closure, Shape, ShapeKind, Value};

static PRELUDE: LazyLock<Vec<Stmt>> = LazyLock::new(|| {
    let tokens = lex(include_str!("prelude.fld"), 1).expect("the prelude lexes");
    parse_prelude(tokens).expect("the prelude parses")
});

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
        let origin = maths::vec2(0.0, 0.0);
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
            knobs: Vec::new(),
            declared: Vec::new(),
        };
        for stmt in PRELUDE.iter() {
            env.statement(stmt)
                .map_err(|e| format!("in the prelude: {e}"))?;
        }
        Ok(Prelude { vars: env.vars })
    }
}

pub const LONGEST_TIME: f32 = 16_777_216.0;

pub struct Declared {
    pub name: String,
    pub slot: usize,
    pub range: Option<(f32, f32)>,
    pub default: Vec<f32>,
}

pub struct Compiled {
    pub tape: Tape,
    pub inputs: Vec<Declared>,
}

impl Compiled {
    pub fn defaults(&self) -> Vec<f32> {
        let mut values = vec![0.0; self.tape.inputs.len()];
        for input in &self.inputs {
            values[input.slot..][..input.default.len()].copy_from_slice(&input.default);
        }
        values
    }
}

pub fn compile(program: &Program, px: f32) -> Result<Compiled, String> {
    let (width, height) = (program.header.width as f32, program.header.height as f32);
    let size = maths::vec2(width, height);
    let prelude = Prelude::load(size)?;
    let across = |length: f32| Range::between(px / 2.0 - length / 2.0, length / 2.0 - px / 2.0);
    let knobs: usize = program
        .body
        .iter()
        .map(|stmt| match &stmt.kind {
            StmtKind::Input { range: Some(_), .. } => 1,
            StmtKind::Input { range: None, .. } => 4,
            _ => 0,
        })
        .sum();
    let mut inputs = vec![
        across(width),
        across(height),
        Range::between(0.0, LONGEST_TIME),
    ];
    inputs.resize(INPUTS + knobs, Range::ANY);
    let mut declared = Vec::new();
    let tape = Tape::record(&inputs, |inputs| {
        let pixel = Pixel {
            pos: maths::vec2(inputs[X], inputs[Y]),
            time: inputs[TIME],
            px: Scalar::from(px),
            size,
        };
        let (canvas, found) = run_with(program, &prelude, pixel, inputs[INPUTS..].to_vec())?;
        declared = found;
        Ok(canvas.channels())
    })?;
    Ok(Compiled {
        tape,
        inputs: declared,
    })
}

#[cfg(test)]
pub fn run(program: &Program, prelude: &Prelude, pixel: Pixel) -> Result<Rgba, String> {
    Ok(run_with(program, prelude, pixel, Vec::new())?.0)
}

fn run_with(
    program: &Program,
    prelude: &Prelude,
    pixel: Pixel,
    knobs: Vec<Scalar>,
) -> Result<(Rgba, Vec<Declared>), String> {
    let mut env = Env {
        pixel,
        globals: &prelude.vars,
        vars: Vec::new(),
        canvas: Rgba::CLEAR,
        knobs,
        declared: Vec::new(),
    };
    for stmt in &program.body {
        env.statement(stmt)?;
    }
    Ok((env.canvas, env.declared))
}

enum Input {
    Num(f32, f32, f32),
    Rgba([f32; 4]),
}

struct Env<'a> {
    pixel: Pixel,
    globals: &'a [(&'a str, Value<'a>)],
    vars: Vec<(&'a str, Value<'a>)>,
    canvas: Rgba,
    knobs: Vec<Scalar>,
    declared: Vec<Declared>,
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
                let value = self.check_input(range, value, stmt.pos)?;
                let value = self.input(name, value);
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

    fn input(&mut self, name: &str, value: Input) -> Value<'a> {
        let first: usize = self.declared.iter().map(|input| input.default.len()).sum();
        let (default, range) = match value {
            Input::Num(n, lo, hi) => (vec![n], Some((lo, hi))),
            Input::Rgba(channels) => (channels.to_vec(), None),
        };
        let Some(knobs) = self.knobs.get(first..first + default.len()) else {
            return match value {
                Input::Num(n, ..) => Value::Num(Scalar::Known(n)),
                Input::Rgba([r, g, b, a]) => Value::Rgba(Rgba {
                    r: r.into(),
                    g: g.into(),
                    b: b.into(),
                    a: a.into(),
                }),
            };
        };
        let bounds = match range {
            Some((lo, hi)) => Range::between(lo, hi),
            None => Range::between(0.0, 1.0),
        };
        for &knob in knobs {
            tape::declare(knob, bounds);
        }
        let value = match knobs {
            &[r, g, b, a] => Value::Rgba(Rgba { r, g, b, a }),
            _ => Value::Num(knobs[0]),
        };
        self.declared.push(Declared {
            name: name.into(),
            slot: INPUTS + first,
            range,
            default,
        });
        value
    }

    fn check_input(
        &self,
        range: &'a Option<(Expr, Expr)>,
        default: Value<'a>,
        pos: Pos,
    ) -> Result<Input, String> {
        let message = match (range, default) {
            (None, Value::Rgba(col)) => match col.valid().channels().map(|c| c.known()) {
                [Some(r), Some(g), Some(b), Some(a)] => return Ok(Input::Rgba([r, g, b, a])),
                _ => "an input's default must be a constant".into(),
            },
            (Some((min, max)), Value::Num(n)) => match (self.expr(min)?, self.expr(max)?) {
                (Value::Num(lo), Value::Num(hi)) => match (lo.known(), hi.known(), n.known()) {
                    (Some(lo), Some(hi), Some(n)) if lo <= n && n <= hi => {
                        return Ok(Input::Num(n + 0.0, lo, hi));
                    }
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
                native::binary(*op, self.expr(left)?, self.expr(right)?)
                    .map_err(|e| at(&e, expr.pos))?
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
                match native::swizzle(&value, field) {
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

        let transform = self.transform(closure);
        let given = transform.map(|_| args.clone());
        let mut env = Env {
            pixel: self.pixel,
            globals: self.globals,
            vars: closure.captured.clone(),
            canvas: Rgba::CLEAR,
            knobs: Vec::new(),
            declared: Vec::new(),
        };
        env.vars.extend(closure.params.iter().copied().zip(args));

        let result = match closure.body {
            Body::Block(body) => env.block(body, pos),
            Body::Expr(body) => env.expr(body),
        }?;
        Ok(match (transform, given) {
            (Some(name), Some(args)) => reached(name, &args, result),
            _ => result,
        })
    }

    fn transform(&self, closure: &Closure<'a>) -> Option<&'a str> {
        self.globals.iter().find_map(|(name, value)| match value {
            Value::Func(known) if std::ptr::eq(&**known, closure) && TRANSFORMS.contains(name) => {
                Some(*name)
            }
            _ => None,
        })
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
        match (name, args) {
            ("dist", [Value::Shape(sh), Value::Vec2(pt)]) => {
                Ok(Value::Num(self.dist(sh, *pt, pos)?))
            }
            _ => native::call(name, args, self.pixel.size).map_err(|e| at(&e, pos)),
        }
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
        Ok(match sh.reach.distance(pt) {
            Some(away) => {
                let floor = away - (away.abs() * REACH_SLACK + 0.001);
                d.hint(away.at_most(0.0).select(f32::NEG_INFINITY, floor))
            }
            None => d,
        })
    }
}

const REACH_SLACK: f32 = 1.0 / 1024.0;

const TRANSFORMS: [&str; 12] = [
    "at",
    "pin",
    "spin",
    "around",
    "mirror",
    "scale",
    "union",
    "intersect",
    "subtract",
    "smooth_union",
    "grow",
    "outline",
];

fn reached<'a>(name: &str, args: &[Value<'a>], result: Value<'a>) -> Value<'a> {
    let Value::Shape(made) = &result else {
        return result;
    };
    let shape = |i: usize| match args.get(i) {
        Some(Value::Shape(sh)) => Some(sh.clone()),
        _ => None,
    };
    let num = |i: usize| match args.get(i) {
        Some(Value::Num(n)) => Some(*n),
        _ => None,
    };
    let Some(first) = shape(0) else {
        return result;
    };
    let inner = first.reach;
    let reach = match name {
        "at" => match (num(1), num(2)) {
            (Some(x), Some(y)) => inner.moved(maths::vec2(x, y)),
            _ => Reach::Everywhere,
        },
        "pin" => match (args.get(1), args.get(2)) {
            (Some(Value::Vec2(anc)), Some(Value::Vec2(to))) => {
                inner.moved(*to - first.bounds.anchor(*anc))
            }
            _ => Reach::Everywhere,
        },
        "spin" | "around" => inner.spun(),
        "mirror" => inner.mirrored(),
        "scale" => num(1).map_or(Reach::Everywhere, |k| inner.scaled(k)),
        "union" => shape(1).map_or(Reach::Everywhere, |other| inner.joined(other.reach)),
        "intersect" | "subtract" => inner,
        "smooth_union" => match (shape(1), num(2).and_then(Scalar::known)) {
            (Some(other), Some(k)) if k > 0.0 => {
                inner.joined(other.reach).widened(Scalar::from(k / 4.0))
            }
            _ => Reach::Everywhere,
        },
        "grow" => num(1).map_or(Reach::Everywhere, |r| inner.widened(r)),
        "outline" => num(1).map_or(Reach::Everywhere, |w| inner.widened(w * 0.5)),
        _ => return result,
    };
    Value::Shape(Rc::new(Shape {
        kind: made.kind.clone(),
        bounds: made.bounds,
        reach,
    }))
}

fn at(message: &str, pos: Pos) -> String {
    format!("{message} at line {}, column {}", pos.line, pos.col)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{self, Canvas, Renderer};
    use std::sync::Arc;
    use crate::syntax::header::parse_header;
    use crate::syntax::parser::parse;
    use crate::tape::{Kernel, Op, Step};

    const SUN: &str = include_str!("../examples/sun.fld");
    const COOKBOOK: &str = include_str!("../docs/COOKBOOK.md");

    fn file(body: &str) -> String {
        format!("~fold v1 256x256\n{body}")
    }

    fn load(src: &str) -> Result<Program, String> {
        let (header, rest) = parse_header(src)?;
        parse(header, lex(rest, 2)?)
    }

    fn recipes() -> impl Iterator<Item = &'static str> {
        COOKBOOK
            .split("```")
            .skip(1)
            .step_by(2)
            .map(str::trim_start)
    }

    fn examples() -> Vec<String> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "fld"))
            .collect();
        paths.sort();
        paths
            .iter()
            .map(|path| std::fs::read_to_string(path).unwrap())
            .collect()
    }

    fn num(src: &str) -> Result<f32, String> {
        let (setup, expr) = src.rsplit_once('\n').unwrap_or(("", src));
        let program = load(&file(&format!("{setup}\nlet result = {expr}")))?;
        let size = maths::vec2(256.0, 256.0);
        let prelude = Prelude::load(size)?;
        let mut env = Env {
            pixel: Pixel {
                pos: maths::vec2(0.0, 0.0),
                time: Scalar::from(0.0),
                px: Scalar::from(1.0),
                size,
            },
            globals: &prelude.vars,
            vars: Vec::new(),
            canvas: Rgba::CLEAR,
            knobs: Vec::new(),
            declared: Vec::new(),
        };
        for stmt in &program.body {
            env.statement(stmt)?;
        }
        match env.lookup("result") {
            Some(Value::Num(n)) => n.known().ok_or("`result` is not known".into()),
            _ => Err("`result` is not a num".into()),
        }
    }

    fn check(cases: &[(&str, f32)]) {
        for &(src, want) in cases {
            let got = num(src).expect(src);
            assert!((got - want).abs() < 1e-3, "{src}: got {got}, want {want}");
        }
    }

    fn canvas_at(src: &str, pos: Vec2) -> Result<[f32; 4], String> {
        let program = load(src)?;
        let size = maths::vec2(program.header.width as f32, program.header.height as f32);
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

    fn canvas(body: &str) -> Result<[f32; 4], String> {
        canvas_at(&file(body), maths::vec2(0.0, 0.0))
    }

    fn hex(rgba: u32) -> [f32; 4] {
        Rgba::hex(rgba).channels().map(|c| c.known().unwrap())
    }

    fn byte_colour([r, g, b, _]: [f32; 4]) -> u32 {
        crate::render::to_u32(r, g, b)
    }

    #[test]
    fn functions() {
        check(&[
            ("func double(x) { return x * 2 }\ndouble(21)", 42.0),
            (
                "func area(w, h) {\n    let a = w * h\n    return a / 2\n}\narea(4, 5)",
                10.0,
            ),
            (
                "let k = 3\nfunc times_k(x) { return x * k }\ntimes_k(5)",
                15.0,
            ),
            (
                "func add(x, by) { return x + by }\n1 |> add(2) |> add(3)",
                6.0,
            ),
            ("func sin(x) { return 7 }\nsin(0)", 7.0),
            ("let double = x => x * 2\ndouble(21)", 42.0),
            ("let k = 3\nlet times_k = x => x * k\ntimes_k(5)", 15.0),
            (
                "func twice(fn, x) { return fn(fn(x)) }\ntwice(x => x + 1, 5)",
                7.0,
            ),
            (
                "func adder(by) { return x => x + by }\nlet add3 = adder(3)\nadd3(4)",
                7.0,
            ),
            (
                "func clamp(x, lo, hi) { return 99 }\nsmoothstep(0, 1, 0.5)",
                0.5,
            ),
            ("func clamp(x, lo, hi) { return 99 }\nclamp(0, 0, 1)", 99.0),
            ("input level: 0..10 = 4\nlevel", 4.0),
        ]);
    }

    #[test]
    fn maths() {
        check(&[
            ("cos(0)", 1.0),
            ("atan2(0, 1)", 0.0),
            ("sqrt(16)", 4.0),
            ("exp(0)", 1.0),
            ("pow(2, 10)", 1024.0),
            ("floor(-1.5)", -2.0),
            ("abs(-3)", 3.0),
            ("min(2, 5)", 2.0),
            ("max(2, 5)", 5.0),
            ("clamp(5, 0, 1)", 1.0),
            ("mix(2, 4, 0.5)", 3.0),
            ("smoothstep(0, 1, 0.5)", 0.5),
            ("length(vec2(3, 4))", 5.0),
            ("DEG * 360", 6.283_185),
            ("(vec2(1, 2) + vec2(3, 4)).y", 6.0),
            ("(vec2(1, 2) * vec2(3, 4)).y", 8.0),
            ("(vec2(6, 8) / vec2(3, 4)).x", 2.0),
            ("(2 * vec2(1, 2)).y", 4.0),
            ("(vec2(1, 2) * 2).y", 4.0),
            ("(vec2(6, 8) / 2).y", 4.0),
            ("vec2(1, 2).yx.x", 2.0),
            ("vec2(1, 2).xx.y", 1.0),
        ]);
    }

    #[test]
    fn shapes() {
        check(&[
            ("dist(circle(10), vec2(30, 40))", 40.0),
            ("dist(rect(20, 10), vec2(15, 0))", 5.0),
            ("dist(rect(20, 10), vec2(0, 0))", -5.0),
            ("dist(segment(vec2(-10, 0), vec2(10, 0)), vec2(0, 7))", 7.0),
            ("dist(segment(vec2(-10, 0), vec2(10, 0)), vec2(13, 4))", 5.0),
            (
                "let ring = shape(pt => dist(circle(10), pt) - 2)\ndist(ring, vec2(20, 0))",
                8.0,
            ),
            (
                "let moved = shape(pt => dist(circle(10), pt - vec2(100, 0)))\ndist(moved, vec2(100, 0))",
                -10.0,
            ),
            (
                "dist(circle(5) |> at(50, 0) |> around(6), vec2(50, 0))",
                -5.0,
            ),
            (
                "dist(circle(5) |> at(50, 0) |> around(6), vec2(25, 43.30127))",
                -5.0,
            ),
            ("anchor(rect(20, 10), vec2(1, 1)).x", 10.0),
            ("anchor(rect(20, 10), vec2(0, -1)).y", -5.0),
            ("anchor(FRAME, TOP_RIGHT).x", 128.0),
            ("dist(circle(10) |> at(100, 0), vec2(100, 0))", -10.0),
            (
                "dist(circle(10) |> at(50, 0) |> mirror, vec2(-50, 0))",
                -10.0,
            ),
            (
                "dist(circle(10) |> at(100, 0) |> spin(90 * DEG), vec2(0, 100))",
                -10.0,
            ),
            (
                "dist(circle(10) |> pin(LEFT, vec2(0, 0)), vec2(10, 0))",
                -10.0,
            ),
            (
                "dist(union(circle(10), circle(10) |> at(100, 0)), vec2(100, 0))",
                -10.0,
            ),
            ("dist(circle(10) |> outline(2), vec2(10, 0))", -1.0),
            ("dist(rounded_rect(40, 20, 5), vec2(0, 0))", -10.0),
            ("cover(circle(10))", 1.0),
            ("cover(circle(10) |> at(100, 0))", 0.0),
        ]);
    }

    #[test]
    fn colours() {
        check(&[
            ("#ff8800.r", 1.0),
            ("#ff8800.g", 136.0 / 255.0),
            ("#ff8800.a", 1.0),
            ("#00000066.a", 0.4),
            ("(#ff8800 * 0.5).a", 0.5),
            ("(#ff8800 * 0.5).r", 1.0),
            ("rgba(1, 0.5, 0, 0.25).g", 0.5),
            ("rgba(1, 0.5, 0, 0.25).a", 0.25),
            ("#ff880066.rgb.a", 1.0),
            ("#ff8800.bgr.r", 0.0),
            ("#ff880066.bgra.a", 0.4),
            ("shade(#808080, 1).r", 1.0),
            ("shade(#808080, -1).r", 0.0),
            ("shade(#000000, 0.3).r", 0.3),
            ("shade(#ffffff, -0.3).r", 0.7),
            ("shade(#80808080, 1).a", 128.0 / 255.0),
        ]);
    }

    #[test]
    fn random_values_are_repeatable_and_in_range() {
        let first = num("hash(vec2(3, 7))").unwrap();
        assert_eq!(num("hash(vec2(3, 7))"), Ok(first));
        assert!((0.0..1.0).contains(&first));
        assert_ne!(num("hash(vec2(7, 3))"), Ok(first));
        assert_eq!(num("hash(vec2(-0, 0))"), num("hash(vec2(0, 0))"));
        let noise = num("noise(vec2(1.5, 2.5))").unwrap();
        assert!((0.0..1.0).contains(&noise), "{noise}");
    }

    #[test]
    fn drawing() {
        for (body, want) in [
            ("let x = 1", [0.0; 4]),
            ("draw #0000ff\ndraw #ff0000", hex(0xff0000ff)),
            ("draw #0000ff\ndraw #ff0000 * 0.5", [0.5, 0.0, 0.5, 1.0]),
            ("draw #ffffff * 3", hex(0xffffffff)),
            (
                "draw #ff0000 * 0.5 + #00ff00 * 0.5 + #0000ff",
                [0.5, 0.5, 1.0, 1.0],
            ),
            ("draw #ff0000\ndraw #ffffff * (0 / 0)", hex(0xff0000ff)),
        ] {
            assert_eq!(canvas(body), Ok(want), "{body}");
        }
    }

    #[test]
    fn valid_files_load() {
        for body in [
            "input accent = #3366ff\ndraw accent",
            "let Sun = 1",
            "func glow(sh, col, amt) { return col }",
            "func f(x) { return x }\nfunc g(x) { return x }",
            "let f = x => x\nlet g = x => x",
        ] {
            canvas(body).expect(body);
        }
    }

    #[test]
    fn errors() {
        for (body, message) in [
            (
                "func again(x) { return again(x) }\nlet y = again(1)",
                "unknown function `again`",
            ),
            (
                "func double(x) { return x * 2 }\nlet y = double(1, 2)",
                "expected 1 arguments, found 2",
            ),
            (
                "func nothing(x) { let y = x }\nlet z = nothing(1)",
                "without `return`",
            ),
            ("let r = 5\nlet y = r(2)", "`r` is a num, not a func"),
            ("let y = circle(10) + 1", "can't use `+` on shape and num"),
            ("let y = 1 / vec2(1, 2)", "can't use `/` on num and vec2"),
            ("let y = vec2(1, 2) + 1", "can't use `+` on vec2 and num"),
            ("let y = sqrt(vec2(1, 2))", "can't take (vec2)"),
            ("let y = vec2(1, 2).z", "a vec2 has no field `z`"),
            ("let y = vec2(1, 2).xyx", "no field `xyx`"),
            ("let y = #ff8800.rg", "an rgba has no field `rg`"),
            (
                "let y = dist(shape(pt => pt), vec2(0, 0))",
                "must return a num, not a vec2",
            ),
            ("draw circle(10)", "`draw` needs an rgba, not a shape"),
            ("OUT = #ffffff", "expected a statement"),
            ("let y = P.x", "unknown name `P`"),
            ("let y = UV.x", "unknown name `UV`"),
            ("input x: 0..1 = 2", "the default 2 is outside 0..1"),
            (
                "input x = 5",
                "without a range must default to an rgba, not a num",
            ),
            (
                "input x: 0..1 = #ffffff",
                "with a range must default to a num, not an rgba",
            ),
            ("let TAU = 3", "`TAU` is all capitals"),
            ("func f(POS) { return 1 }", "`POS` is all capitals"),
            ("let a = 1\nlet a = 2", "`a` is already defined"),
            ("func f(x, x) { return x }", "`x` is already a parameter"),
            (
                "func f(x) { let x = 1\nreturn x }",
                "`x` is already defined",
            ),
            (
                "let height = 1\nfunc f(height) { return height }",
                "`height` is already defined",
            ),
            (
                "let a = 1\nfunc f(x) { let a = x\nreturn a }",
                "`a` is already defined",
            ),
            (
                "let k = 2\nlet double = k => k * 2",
                "`k` is already defined",
            ),
            (
                "func f(pt) { return shape(pt => 1) }",
                "`pt` is already defined",
            ),
        ] {
            let error = canvas(body).expect_err(body);
            assert!(error.contains(message), "{body}: {error}");
        }
    }

    #[test]
    fn other_versions_are_refused() {
        for (src, message) in [
            ("~fold v0 256x256\n", "this engine supports v1"),
            ("~fold v2 256x256\n", "this file is Fold v2"),
        ] {
            let error = canvas_at(src, maths::vec2(0.0, 0.0)).expect_err(src);
            assert!(error.contains(message), "{src}: {error}");
        }
    }

    #[test]
    fn sun_example_renders() {
        let centre = canvas_at(SUN, maths::vec2(0.0, 0.0)).unwrap();
        assert_eq!(byte_colour(centre), 0xffaa00);
        let corner = canvas_at(SUN, maths::vec2(-120.0, -120.0)).unwrap();
        assert_eq!(corner[3], 1.0);
        assert_ne!(byte_colour(corner), 0xffaa00);
    }

    fn tape_of(src: &str) -> Result<Tape, String> {
        let tape = compile(&load(src)?, 1.0)?.tape;
        Ok(tape.specialise(&tape.inputs))
    }

    fn same_on_tape(src: &str) {
        let program = load(src).expect(src);
        for px in [1.0, 2.0] {
            same_at_scale(src, &program, px);
        }
    }

    fn same_at_scale(src: &str, program: &Program, px: f32) {
        let compiled = compile(program, px).expect(src);
        let mut values = compiled.defaults();
        let tape = compiled.tape;
        let (units_x, units_y) = (program.header.width as f32, program.header.height as f32);
        let canvas = Canvas {
            width: (units_x / px).round() as usize,
            height: (units_y / px).round() as usize,
            px,
            middle: [0.0, 0.0],
        };
        let (width, height) = (canvas.width, canvas.height);
        let size = maths::vec2(units_x, units_y);
        let prelude = Prelude::load(size).unwrap();
        let spacing = (width.max(height) / 24).max(1);
        let reference = |col: usize, row: usize, time: f32| {
            let pixel = Pixel {
                pos: maths::vec2(
                    (col as f32 + 0.5 - width as f32 / 2.0) * px,
                    (row as f32 + 0.5 - height as f32 / 2.0) * px,
                ),
                time: Scalar::from(time),
                px: Scalar::from(px),
                size,
            };
            run(program, &prelude, pixel).expect(src).channels()
        };
        let shared = Arc::new(compile(program, px).unwrap().tape);
        let mut folded = Renderer::new(shared.clone(), canvas, &values, false, 0..height);
        let mut live = Renderer::new(shared, canvas, &values, true, 0..height);
        for time in [0.0, 1.3] {
            values[TIME] = time;
            let wanted = |row: usize| row.is_multiple_of(spacing);
            render::trace(&tape, canvas, &values, wanted, |col, row, got| {
                if !col.is_multiple_of(spacing) {
                    return;
                }
                let want = reference(col, row, time);
                for (want, got) in want.iter().zip(got) {
                    let want = want.known().unwrap();
                    assert_eq!(want.to_bits(), got.to_bits(), "at ({col}, {row}) px {px}: {src}");
                }
            });
            for renderer in [&mut folded, &mut live] {
                let pixels = renderer.frame(time).to_vec();
                for row in (0..height).step_by(spacing) {
                    for col in (0..width).step_by(spacing) {
                        let want = reference(col, row, time).map(|c| c.known().unwrap());
                        let got = pixels[row * width + col];
                        assert_eq!(byte_colour(want), got, "at ({col}, {row}) px {px}: {src}");
                    }
                }
            }
        }
    }

    #[test]
    fn tape_matches_the_reference_bit_for_bit() {
        for example in examples() {
            same_on_tape(&example);
        }
        for recipe in recipes() {
            same_on_tape(&file(recipe));
        }
    }

    #[test]
    fn checks_that_can_fire_stay_exact() {
        for body in [
            "draw #ffffff * sqrt(POS.x)",
            "draw #ffffff * (POS.x * 0)",
            "draw rgba(1, 1, 1, POS.x / 100)",
            "draw #ffffff * (POS.y / 40)\ndraw #ff000080 + #00ff0080 * (POS.x / 64)",
            "draw #3366ff * -1",
            "draw FRAME |> fill(#101820)\ndraw #ffffff * (0 / POS.x)",
        ] {
            same_on_tape(&file(body));
        }
    }

    #[test]
    fn errors_are_found_when_compiling() {
        let src = file("let sun = circle(10)\ndraw sun |> fill(#ffaa00) + 1");
        let error = tape_of(&src).unwrap_err();
        assert!(error.contains("can't use `+` on rgba and num"), "{error}");
    }

    fn count(tape: &Tape, op: Op) -> usize {
        tape.steps.iter().filter(|step| step.op == op).count()
    }

    #[test]
    fn constant_work_leaves_the_tape() {
        let tape = tape_of(&file(
            "let k = sin(1) * 20 + 3
draw #ffffff * k",
        ))
        .unwrap();
        assert_eq!(count(&tape, Op::Sin), 0);
    }

    #[test]
    fn a_solid_background_needs_no_steps() {
        let tape = tape_of(&file("draw FRAME |> fill(#101820)")).unwrap();
        assert!(tape.steps.is_empty());
    }

    #[test]
    fn repeated_work_is_shared() {
        let tape = tape_of(&file("draw #ffffff * (sqrt(POS.x) + sqrt(POS.x))")).unwrap();
        assert_eq!(count(&tape, Op::Sqrt), 1);
    }

    #[test]
    fn work_runs_at_the_level_it_depends_on() {
        let src = "draw #ffffff * (sin(TIME) + sin(POS.x) * cos(POS.y) * POS.x)";
        let kernel = Kernel::new(&tape_of(&file(src)).unwrap());
        let ops = |steps: &[Step]| steps.iter().map(|step| step.op).collect::<Vec<_>>();
        assert_eq!(ops(&kernel.frame), [Op::Sin]);
        assert_eq!(ops(&kernel.row), [Op::Cos]);
        assert_eq!(ops(&kernel.batch.columns), [Op::Sin]);
        assert!(!ops(&kernel.batch.pixel).contains(&Op::Sin));
        assert!(kernel.changes);
    }

    #[test]
    fn still_pictures_do_not_change() {
        let kernel = Kernel::new(&tape_of(&file("draw circle(40) |> fill(#ffaa00)")).unwrap());
        assert!(!kernel.changes);
    }

    #[test]
    fn impossible_checks_leave_the_tape() {
        let tape = tape_of(SUN).unwrap();
        for check in [Op::IsNan, Op::Select, Op::Clamp] {
            assert_eq!(count(&tape, check), 0, "{check:?}");
        }
        assert!(
            tape.steps.len() <= 35,
            "sun.fld has {} steps",
            tape.steps.len()
        );
    }
}
