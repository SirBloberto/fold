use super::float::{atan2, clamp, cos, exp, flag, hash, max, min, pow, sin, to_linear, to_srgb};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Sqrt,
    Abs,
    Floor,
    Sin,
    Cos,
    Exp,
    Pow,
    Atan2,
    Min,
    Max,
    Clamp,
    ToLinear,
    ToSrgb,
    Hash,
    IsNan,
    AtMost,
    Select,
    Hint,
}

impl Op {
    #[inline(always)]
    pub fn eval(self, a: f32, b: f32, c: f32) -> f32 {
        match self {
            Op::Add => a + b,
            Op::Sub => a - b,
            Op::Mul => a * b,
            Op::Div => a / b,
            Op::Neg => -a,
            Op::Sqrt => a.sqrt(),
            Op::Abs => a.abs(),
            Op::Floor => a.floor(),
            Op::Sin => sin(a),
            Op::Cos => cos(a),
            Op::Exp => exp(a),
            Op::Pow => pow(a, b),
            Op::Atan2 => atan2(a, b),
            Op::Min => min(a, b),
            Op::Max => max(a, b),
            Op::Clamp => clamp(a, b, c),
            Op::ToLinear => to_linear(a),
            Op::ToSrgb => to_srgb(a),
            Op::Hash => hash(a, b),
            Op::IsNan => flag(a.is_nan()),
            Op::AtMost => flag(a <= b),
            Op::Select => {
                if a != 0.0 {
                    b
                } else {
                    c
                }
            }
            Op::Hint => a,
        }
    }
}
