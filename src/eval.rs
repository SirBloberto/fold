mod native;
mod value;

use std::rc::Rc;
use std::sync::LazyLock;

use crate::maths::{self, Rgba, Vec2, shape};
use crate::syntax::ast::{Expr, ExprKind, Program, Stmt, StmtKind};
use crate::syntax::lexer::lex;
use crate::syntax::parser::parse_prelude;
use crate::syntax::token::Pos;
use crate::tape::{Range, Scalar, Tape};
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
        };
        for stmt in PRELUDE.iter() {
            env.statement(stmt)
                .map_err(|e| format!("in the prelude: {e}"))?;
        }
        Ok(Prelude { vars: env.vars })
    }
}

pub const LONGEST_TIME: f32 = 16_777_216.0;

pub fn compile(program: &Program, px: f32) -> Result<Tape, String> {
    let (width, height) = (program.header.width as f32, program.header.height as f32);
    let size = maths::vec2(width, height);
    let prelude = Prelude::load(size)?;
    let across = |length: f32| Range::between(px / 2.0 - length / 2.0, length / 2.0 - px / 2.0);
    let inputs = [
        across(width),
        across(height),
        Range::between(0.0, LONGEST_TIME),
    ];
    Tape::record(&inputs, |inputs| {
        let pixel = Pixel {
            pos: maths::vec2(inputs[0], inputs[1]),
            time: inputs[2],
            px: Scalar::from(px),
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
        Ok(d)
    }
}

fn at(message: &str, pos: Pos) -> String {
    format!("{message} at line {}, column {}", pos.line, pos.col)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::header::parse_header;
    use crate::syntax::parser::parse;
    use crate::tape::{Op, Step};

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

    #[test]
    fn cookbook_recipes_render() {
        for recipe in recipes() {
            for pos in [
                maths::vec2(0.0, 0.0),
                maths::vec2(40.0, -30.0),
                maths::vec2(-120.0, 110.0),
            ] {
                if let Err(e) = canvas_at(&file(recipe), pos) {
                    panic!("{e}\n{recipe}");
                }
            }
        }
    }

    fn tape_of(src: &str) -> Result<Tape, String> {
        compile(&load(src)?, 1.0)
    }

    fn same_on_tape(src: &str) {
        let program = load(src).unwrap();
        let tape = compile(&program, 1.0).unwrap();
        let size = maths::vec2(program.header.width as f32, program.header.height as f32);
        let prelude = Prelude::load(size).unwrap();
        let mut slots = tape.slots();
        for time in [0.0, 1.3] {
            for y in (-128..128).step_by(9) {
                for x in (-128..128).step_by(9) {
                    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
                    let pixel = Pixel {
                        pos: maths::vec2(x, y),
                        time: Scalar::from(time),
                        px: Scalar::from(1.0),
                        size,
                    };
                    let want = run(&program, &prelude, pixel).unwrap().channels();
                    let got = tape.run(&mut slots, &[x, y, time]);
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
        same_on_tape(SUN);
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

    #[test]
    fn constant_work_leaves_the_tape() {
        let tape = tape_of(&file("let k = sin(1) * 20 + 3\ndraw #ffffff * k")).unwrap();
        assert!(tape.steps.iter().all(|step| step.op != Op::Sin));
    }

    #[test]
    fn a_solid_background_is_a_constant() {
        let tape = tape_of(&file("draw FRAME |> fill(#101820)")).unwrap();
        let constant = |slot: &u32| tape.constants.iter().any(|(at, _)| at == slot);
        assert!(tape.outputs.iter().all(constant));
    }

    #[test]
    fn impossible_checks_leave_the_tape() {
        let tape = tape_of(SUN).unwrap();
        let checks = |step: &Step| matches!(step.op, Op::IsNan | Op::Select | Op::Clamp);
        assert!(!tape.steps.iter().any(checks));
        assert!(
            tape.steps.len() <= 44,
            "sun.fld has {} steps",
            tape.steps.len()
        );
    }
}
