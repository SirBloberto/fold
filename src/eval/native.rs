use super::value::{ShapeKind, Value, new_shape};
use crate::maths::shape::{Bounds, Reach};
use crate::maths::{self, Rgba};
use crate::syntax::ast::BinOp;
use crate::tape::Scalar;

pub fn call<'a>(name: &str, args: &[Value<'a>], size: maths::Vec2) -> Result<Value<'a>, String> {
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

        ("vec2", [Num(x), Num(y)]) => Vec2(maths::vec2(*x, *y)),

        ("rgba", [Num(r), Num(g), Num(b), Num(a)]) => Rgba(maths::Rgba::from_srgb(*r, *g, *b, *a)),

        ("hash", [Vec2(pt)]) => Num(maths::hash(*pt)),

        ("circle", [Num(r)]) => {
            let half = maths::vec2(*r, *r);
            new_shape(ShapeKind::Circle(*r), Bounds::around(half), Reach::around(half))
        }
        ("rect", [Num(w), Num(h)]) => {
            let half = maths::vec2(*w / 2.0, *h / 2.0);
            new_shape(ShapeKind::Rect(half), Bounds::around(half), Reach::around(half))
        }
        ("segment", [Vec2(from), Vec2(to)]) => {
            let bounds = Bounds::between(*from, *to);
            let reach = Reach::Box {
                min: bounds.min,
                max: bounds.max,
            };
            new_shape(ShapeKind::Segment(*from, *to), bounds, reach)
        }
        ("shape", [Func(fn_)]) => {
            let bounds = Bounds::around(size * 0.5);
            new_shape(ShapeKind::Custom(fn_.clone()), bounds, Reach::Everywhere)
        }
        ("anchor", [Shape(sh), Vec2(anc)]) => Vec2(sh.bounds.anchor(*anc)),

        _ => {
            let types: Vec<&str> = args.iter().map(Value::type_name).collect();
            return Err(format!(
                "unknown function `{name}`, or it can't take ({})",
                types.join(", ")
            ));
        }
    };
    Ok(result)
}

pub fn binary<'a>(op: BinOp, left: Value<'a>, right: Value<'a>) -> Result<Value<'a>, String> {
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

pub fn swizzle<'a>(value: &Value<'a>, field: &str) -> Option<Value<'a>> {
    match value {
        Value::Vec2(v) => {
            let parts = pick(field, |c| match c {
                'x' => Some(v.x),
                'y' => Some(v.y),
                _ => None,
            })?;
            match parts[..] {
                [n] => Some(Value::Num(n)),
                [x, y] => Some(Value::Vec2(maths::vec2(x, y))),
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
