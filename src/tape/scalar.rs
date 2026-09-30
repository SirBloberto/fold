use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

use super::Op;
use super::record;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scalar {
    Known(f32),
    Slot(u32),
}

impl Scalar {
    pub fn known(self) -> Option<f32> {
        match self {
            Scalar::Known(n) => Some(n),
            Scalar::Slot(_) => None,
        }
    }

    fn apply(op: Op, args: [Scalar; 3]) -> Scalar {
        if let [Scalar::Known(a), Scalar::Known(b), Scalar::Known(c)] = args {
            return Scalar::Known(op.eval(a, b, c));
        }
        record::trace(op, args)
    }

    fn unary(self, op: Op) -> Scalar {
        Scalar::apply(op, [self, ZERO, ZERO])
    }

    fn binary(self, op: Op, other: impl Into<Scalar>) -> Scalar {
        Scalar::apply(op, [self, other.into(), ZERO])
    }

    pub fn sqrt(self) -> Scalar {
        self.unary(Op::Sqrt)
    }

    pub fn abs(self) -> Scalar {
        self.unary(Op::Abs)
    }

    pub fn floor(self) -> Scalar {
        self.unary(Op::Floor)
    }

    pub fn sin(self) -> Scalar {
        self.unary(Op::Sin)
    }

    pub fn cos(self) -> Scalar {
        self.unary(Op::Cos)
    }

    pub fn exp(self) -> Scalar {
        self.unary(Op::Exp)
    }

    pub fn to_linear(self) -> Scalar {
        self.unary(Op::ToLinear)
    }

    pub fn to_srgb(self) -> Scalar {
        self.unary(Op::ToSrgb)
    }

    pub fn is_nan(self) -> Scalar {
        self.unary(Op::IsNan)
    }

    pub fn powf(self, by: impl Into<Scalar>) -> Scalar {
        self.binary(Op::Pow, by)
    }

    pub fn atan2(self, x: impl Into<Scalar>) -> Scalar {
        self.binary(Op::Atan2, x)
    }

    pub fn min(self, other: impl Into<Scalar>) -> Scalar {
        self.binary(Op::Min, other)
    }

    pub fn max(self, other: impl Into<Scalar>) -> Scalar {
        self.binary(Op::Max, other)
    }

    pub fn at_most(self, other: impl Into<Scalar>) -> Scalar {
        self.binary(Op::AtMost, other)
    }

    pub fn hash(self, y: Scalar) -> Scalar {
        self.binary(Op::Hash, y)
    }

    pub fn clamp(self, lo: impl Into<Scalar>, hi: impl Into<Scalar>) -> Scalar {
        Scalar::apply(Op::Clamp, [self, lo.into(), hi.into()])
    }

    pub fn hint(self, floor: Scalar) -> Scalar {
        match self {
            Scalar::Known(_) => self,
            Scalar::Slot(_) => self.binary(Op::Hint, floor),
        }
    }

    pub fn select(self, yes: impl Into<Scalar>, no: impl Into<Scalar>) -> Scalar {
        Scalar::apply(Op::Select, [self, yes.into(), no.into()])
    }
}

const ZERO: Scalar = Scalar::Known(0.0);

impl From<f32> for Scalar {
    fn from(n: f32) -> Scalar {
        Scalar::Known(n)
    }
}

impl fmt::Display for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Scalar::Known(n) => write!(f, "{n}"),
            Scalar::Slot(_) => write!(f, "a per-pixel value"),
        }
    }
}

impl Neg for Scalar {
    type Output = Scalar;
    fn neg(self) -> Scalar {
        self.unary(Op::Neg)
    }
}

macro_rules! operator {
    ($trait:ident, $method:ident, $op:expr) => {
        impl<T: Into<Scalar>> $trait<T> for Scalar {
            type Output = Scalar;
            fn $method(self, other: T) -> Scalar {
                self.binary($op, other)
            }
        }

        impl $trait<Scalar> for f32 {
            type Output = Scalar;
            fn $method(self, other: Scalar) -> Scalar {
                Scalar::from(self).binary($op, other)
            }
        }
    };
}

operator!(Add, add, Op::Add);
operator!(Sub, sub, Op::Sub);
operator!(Mul, mul, Op::Mul);
operator!(Div, div, Op::Div);
