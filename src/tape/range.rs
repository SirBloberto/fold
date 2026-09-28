use super::Op;
use super::float;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub lo: f32,
    pub hi: f32,
    pub nan: bool,
    pub minus_zero: bool,
}

impl Range {
    pub const ANY: Range = Range {
        lo: f32::NEG_INFINITY,
        hi: f32::INFINITY,
        nan: true,
        minus_zero: true,
    };

    pub fn between(lo: f32, hi: f32) -> Range {
        Range {
            lo: if lo.is_nan() { f32::NEG_INFINITY } else { lo },
            hi: if hi.is_nan() { f32::INFINITY } else { hi },
            nan: false,
            minus_zero: false,
        }
    }

    pub fn of(n: f32) -> Range {
        if n.is_nan() {
            return Range::ANY;
        }
        Range {
            lo: n,
            hi: n,
            nan: false,
            minus_zero: n == 0.0 && n.is_sign_negative(),
        }
    }

    pub fn single(self) -> Option<f32> {
        let exact = self.lo == self.hi && !self.nan && !(self.lo == 0.0 && self.minus_zero);
        exact.then_some(self.lo + 0.0)
    }

    pub fn is_finite(self) -> bool {
        !self.nan && self.lo.is_finite() && self.hi.is_finite()
    }

    pub fn at_least_zero(self) -> bool {
        !self.nan && self.lo >= 0.0 && !self.minus_zero
    }

    fn has_zero(self) -> bool {
        self.lo <= 0.0 && self.hi >= 0.0
    }

    fn has_infinity(self) -> bool {
        self.lo.is_infinite() || self.hi.is_infinite()
    }

    fn widen(self) -> Range {
        Range {
            lo: self.lo.next_down(),
            hi: self.hi.next_up(),
            ..self
        }
    }

    fn corners(a: Range, b: Range, f: impl Fn(f32, f32) -> f32) -> Range {
        let values = [f(a.lo, b.lo), f(a.lo, b.hi), f(a.hi, b.lo), f(a.hi, b.hi)];
        if values.iter().any(|v| v.is_nan()) {
            return Range::between(f32::NEG_INFINITY, f32::INFINITY);
        }
        let lo = values.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        Range::between(lo, hi)
    }
}

pub fn square(a: Range) -> Range {
    let far = a.lo.abs().max(a.hi.abs());
    let near = if a.has_zero() {
        0.0
    } else {
        a.lo.abs().min(a.hi.abs())
    };
    Range {
        nan: a.nan,
        ..Range::between(near * near, far * far)
    }
}

pub fn of_op(op: Op, [a, b, c]: [Range; 3]) -> Range {
    let mixed_signs = !(a.at_least_zero() && b.at_least_zero());
    match op {
        Op::Add => {
            let opposite = (a.hi == f32::INFINITY && b.lo == f32::NEG_INFINITY)
                || (a.lo == f32::NEG_INFINITY && b.hi == f32::INFINITY);
            let sum = Range::between(a.lo + b.lo, a.hi + b.hi);
            Range {
                nan: opposite || a.nan || b.nan,
                minus_zero: a.minus_zero && b.minus_zero,
                ..sum
            }
        }
        Op::Sub => {
            let same = (a.hi == f32::INFINITY && b.hi == f32::INFINITY)
                || (a.lo == f32::NEG_INFINITY && b.lo == f32::NEG_INFINITY);
            let difference = Range::between(a.lo - b.hi, a.hi - b.lo);
            Range {
                nan: same || a.nan || b.nan,
                minus_zero: a.minus_zero,
                ..difference
            }
        }
        Op::Mul => Range {
            nan: a.nan
                || b.nan
                || (a.has_zero() && b.has_infinity())
                || (b.has_zero() && a.has_infinity()),
            minus_zero: mixed_signs,
            ..Range::corners(a, b, |x, y| x * y)
        },
        Op::Div if b.has_zero() => Range::ANY,
        Op::Div => Range {
            nan: a.nan || b.nan || (a.has_infinity() && b.has_infinity()),
            minus_zero: mixed_signs,
            ..Range::corners(a, b, |x, y| x / y)
        },
        Op::Neg => Range {
            lo: -a.hi,
            hi: -a.lo,
            nan: a.nan,
            minus_zero: a.has_zero(),
        },
        Op::Sqrt => Range {
            lo: a.lo.max(0.0).sqrt(),
            hi: a.hi.sqrt(),
            nan: a.nan || a.lo < 0.0,
            minus_zero: a.minus_zero,
        },
        Op::Abs => {
            let far = a.lo.abs().max(a.hi.abs());
            let near = if a.has_zero() {
                0.0
            } else {
                a.lo.abs().min(a.hi.abs())
            };
            Range {
                nan: a.nan,
                ..Range::between(near, far)
            }
        }
        Op::Floor => Range {
            lo: a.lo.floor(),
            hi: a.hi.floor(),
            ..a
        },
        Op::Sin | Op::Cos => Range {
            nan: a.nan || !a.is_finite(),
            minus_zero: true,
            ..Range::between(-1.0, 1.0).widen()
        },
        Op::Exp => Range {
            lo: below(float::exp(a.lo)).max(0.0),
            hi: if a.hi <= 0.0 {
                1.0
            } else {
                above(float::exp(a.hi))
            },
            nan: a.nan,
            minus_zero: false,
        },
        Op::Atan2 => Range {
            nan: a.nan || b.nan,
            minus_zero: true,
            ..Range::between(-std::f32::consts::PI, std::f32::consts::PI).widen()
        },
        Op::Min => Range {
            lo: a.lo.min(b.lo),
            hi: if a.nan || b.nan {
                a.hi.max(b.hi)
            } else {
                a.hi.min(b.hi)
            },
            nan: a.nan && b.nan,
            minus_zero: a.minus_zero || b.minus_zero,
        },
        Op::Max => Range {
            lo: if a.nan || b.nan {
                a.lo.min(b.lo)
            } else {
                a.lo.max(b.lo)
            },
            hi: a.hi.max(b.hi),
            nan: a.nan && b.nan,
            minus_zero: a.minus_zero || b.minus_zero,
        },
        Op::Clamp if b.nan || c.nan => Range::ANY,
        Op::Clamp => Range {
            lo: c.lo.min(a.lo.max(b.lo)),
            hi: b.hi.max(a.hi.min(c.hi)),
            nan: a.nan,
            minus_zero: a.minus_zero || b.minus_zero || c.minus_zero,
        },
        Op::ToLinear | Op::ToSrgb if a.lo >= 0.0 && a.hi <= 1.0 && !a.nan => Range {
            minus_zero: a.minus_zero,
            ..Range::between(0.0, 1.0).widen()
        },
        Op::Hash => Range::between(0.0, 1.0),
        Op::IsNan | Op::AtMost => Range::between(0.0, 1.0),
        Op::Select => Range {
            lo: b.lo.min(c.lo),
            hi: b.hi.max(c.hi),
            nan: b.nan || c.nan,
            minus_zero: b.minus_zero || c.minus_zero,
        },
        Op::Pow | Op::ToLinear | Op::ToSrgb => Range::ANY,
    }
}

fn below(x: f32) -> f32 {
    (0..4).fold(x, |x, _| x.next_down())
}

fn above(x: f32) -> f32 {
    (0..4).fold(x, |x, _| x.next_up())
}
