#[derive(Clone, Copy, Debug, PartialEq)]
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
}

impl Op {
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
            Op::Sin => a.sin(),
            Op::Cos => a.cos(),
            Op::Exp => a.exp(),
            Op::Pow => a.powf(b),
            Op::Atan2 => a.atan2(b),
            Op::Min => a.min(b),
            Op::Max => a.max(b),
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
        }
    }
}

fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

fn flag(yes: bool) -> f32 {
    if yes { 1.0 } else { 0.0 }
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(light: f32) -> f32 {
    if light <= 0.003_130_8 {
        light * 12.92
    } else {
        1.055 * light.powf(1.0 / 2.4) - 0.055
    }
}

fn hash(x: f32, y: f32) -> f32 {
    let x = (x + 0.0).to_bits();
    let y = (y + 0.0).to_bits();
    let bits = scramble(x ^ scramble(y ^ 0x9e37_79b9));
    (bits >> 8) as f32 / 16_777_216.0
}

fn scramble(mut bits: u32) -> u32 {
    bits ^= bits >> 16;
    bits = bits.wrapping_mul(0x7feb_352d);
    bits ^= bits >> 15;
    bits = bits.wrapping_mul(0x846c_a68b);
    bits ^= bits >> 16;
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_survives_the_round_trip() {
        for byte in 0..=255u32 {
            let there = to_linear(byte as f32 / 255.0);
            let back = (to_srgb(there) * 255.0).round() as u32;
            assert_eq!(back, byte);
        }
    }
}
