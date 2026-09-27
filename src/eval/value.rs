use std::rc::Rc;

use crate::maths::shape::Bounds;
use crate::maths::{Rgba, Vec2};
use crate::syntax::ast::{Expr, Stmt};
use crate::tape::Scalar;

#[derive(Clone)]
pub enum Value<'a> {
    Num(Scalar),
    Vec2(Vec2),
    Rgba(Rgba),
    Func(Rc<Closure<'a>>),
    Shape(Rc<Shape<'a>>),
}

pub struct Closure<'a> {
    pub params: Vec<&'a str>,
    pub body: Body<'a>,
    pub captured: Vec<(&'a str, Value<'a>)>,
}

pub enum Body<'a> {
    Block(&'a [Stmt]),
    Expr(&'a Expr),
}

pub struct Shape<'a> {
    pub kind: ShapeKind<'a>,
    pub bounds: Bounds,
}

pub enum ShapeKind<'a> {
    Circle(Scalar),
    Rect(Vec2),
    Segment(Vec2, Vec2),
    Custom(Rc<Closure<'a>>),
}

pub fn new_shape<'a>(kind: ShapeKind<'a>, bounds: Bounds) -> Value<'a> {
    Value::Shape(Rc::new(Shape { kind, bounds }))
}

impl Value<'_> {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Num(_) => "num",
            Value::Vec2(_) => "vec2",
            Value::Rgba(_) => "rgba",
            Value::Func(_) => "func",
            Value::Shape(_) => "shape",
        }
    }

    pub fn a_type(&self) -> String {
        let article = if matches!(self, Value::Rgba(_)) {
            "an"
        } else {
            "a"
        };
        format!("{article} {}", self.type_name())
    }
}
