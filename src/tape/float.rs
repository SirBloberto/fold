use std::f32::consts::{FRAC_2_PI, FRAC_PI_2, FRAC_PI_4, LOG2_E, PI, SQRT_2};

const HALF_PI: [f32; 3] = [1.570_312_5, 4.837_512_969_970_703e-4, 7.549_79e-8];
const LN_2: [f32; 2] = [0.693_359_375, -2.121_944_4e-4];

pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

pub fn flag(yes: bool) -> f32 {
    if yes { 1.0 } else { 0.0 }
}

#[inline(always)]
pub fn sin(x: f32) -> f32 {
    sine(x, 0.0)
}

#[inline(always)]
pub fn cos(x: f32) -> f32 {
    sine(x, 1.0)
}

#[inline(always)]
fn sine(x: f32, quarter: f32) -> f32 {
    let k = (x * FRAC_2_PI + 0.5).floor();
    let r = ((x - k * HALF_PI[0]) - k * HALF_PI[1]) - k * HALF_PI[2];
    let z = r * r;
    let s = ((-1.951_529_6e-4 * z + 8.332_161e-3) * z - 1.666_665_5e-1) * z * r + r;
    let c = ((2.443_315_7e-5 * z - 1.388_731_6e-3) * z + 4.166_664_6e-2) * z * z - 0.5 * z + 1.0;
    let q = k + quarter;
    let turn = q - 4.0 * (q * 0.25).floor();
    let v = if turn == 1.0 || turn == 3.0 { c } else { s };
    if turn >= 2.0 { -v } else { v }
}

#[inline(always)]
pub fn exp(x: f32) -> f32 {
    let x = clamp(x, -104.0, 89.0);
    let k = (x * LOG2_E + 0.5).floor();
    let r = (x - k * LN_2[0]) - k * LN_2[1];
    let z = r * r;
    let p = (((((1.987_569_1e-4 * r + 1.398_199_9e-3) * r + 8.333_452e-3) * r + 4.166_579_6e-2)
        * r
        + 1.666_666_5e-1)
        * r
        + 0.5)
        * z
        + r
        + 1.0;
    let half = (k * 0.5).floor();
    p * power_of_two(half) * power_of_two(k - half)
}

#[inline(always)]
fn power_of_two(n: f32) -> f32 {
    f32::from_bits((n + 8_388_735.0).to_bits() << 23)
}

#[inline(always)]
pub fn log(x: f32) -> f32 {
    let tiny = x < f32::MIN_POSITIVE;
    let scaled = if tiny { x * 8_388_608.0 } else { x };
    let bits = scaled.to_bits();
    let m = f32::from_bits((bits & 0x007f_ffff) | 0x3f80_0000);
    let big = m > SQRT_2;
    let m = if big { m * 0.5 } else { m };
    let e = ((bits >> 23) & 0xff) as f32 - 127.0 + flag(big) - 23.0 * flag(tiny);
    let f = m - 1.0;
    let z = f * f;
    let p = ((((((((7.037_683_6e-2 * f - 1.151_461e-1) * f + 1.167_699_9e-1) * f
        - 1.242_014_1e-1)
        * f
        + 1.424_932_3e-1)
        * f
        - 1.666_805_7e-1)
        * f
        + 2.000_071_5e-1)
        * f
        - 2.499_999_4e-1)
        * f
        + 3.333_333_1e-1)
        * f
        * z;
    let v = f + (p + LN_2[1] * e - 0.5 * z) + LN_2[0] * e;
    if x == f32::INFINITY {
        x
    } else if x > 0.0 {
        v
    } else if x == 0.0 {
        f32::NEG_INFINITY
    } else {
        f32::NAN
    }
}

#[inline(always)]
pub fn pow(x: f32, y: f32) -> f32 {
    let r = exp(y * log(x.abs()));
    let whole = y.floor() == y;
    let odd = whole && (y * 0.5).floor() * 2.0 != y;
    let signed = if x.is_sign_negative() && odd { -r } else { r };
    if y == 0.0 || x == 1.0 {
        1.0
    } else if x < 0.0 && !whole {
        f32::NAN
    } else {
        signed
    }
}

#[inline(always)]
pub fn atan2(y: f32, x: f32) -> f32 {
    let (ax, ay) = (x.abs(), y.abs());
    let big = if ax > ay { ax } else { ay };
    let small = if ax > ay { ay } else { ax };
    let t = if big == 0.0 {
        0.0
    } else if big == small {
        1.0
    } else {
        small / big
    };
    let far = t > 0.414_213_57;
    let u = if far { (t - 1.0) / (t + 1.0) } else { t };
    let z = u * u;
    let p =
        (((8.053_744_5e-2 * z - 1.387_768_6e-1) * z + 1.997_771_1e-1) * z - 3.333_295e-1) * z * u
            + u;
    let a = if far { p + FRAC_PI_4 } else { p };
    let a = if ay > ax { FRAC_PI_2 - a } else { a };
    let a = if x.is_sign_negative() { PI - a } else { a };
    let a = if y.is_sign_negative() { -a } else { a };
    if x.is_nan() || y.is_nan() {
        f32::NAN
    } else {
        a
    }
}

#[inline(always)]
pub fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        pow((c + 0.055) / 1.055, 2.4)
    }
}

#[inline(always)]
pub fn to_srgb(light: f32) -> f32 {
    if light <= 0.003_130_8 {
        light * 12.92
    } else {
        1.055 * pow(light, 1.0 / 2.4) - 0.055
    }
}

#[inline(always)]
pub fn hash(x: f32, y: f32) -> f32 {
    let x = (x + 0.0).to_bits();
    let y = (y + 0.0).to_bits();
    let bits = scramble(x ^ scramble(y ^ 0x9e37_79b9));
    (bits >> 8) as f32 / 16_777_216.0
}

#[inline(always)]
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

    fn ulps(got: f32, want: f64) -> u32 {
        let want = want as f32;
        let order = |x: f32| {
            let bits = x.to_bits() as i64;
            if bits < 0x8000_0000 {
                bits
            } else {
                0x8000_0000 - bits
            }
        };
        (order(got) - order(want)).unsigned_abs() as u32
    }

    fn sweep(lo: f32, hi: f32, n: u32) -> impl Iterator<Item = f32> {
        (0..=n).map(move |i| lo + (hi - lo) * (i as f32 / n as f32))
    }

    fn unit_floats() -> impl Iterator<Item = f32> {
        (0..=1.0f32.to_bits()).step_by(997).map(f32::from_bits)
    }

    #[test]
    fn sine_and_cosine_are_accurate() {
        for x in sweep(-8192.0, 8192.0, 1_000_000) {
            assert!((sin(x) as f64 - (x as f64).sin()).abs() < 1e-7, "sin({x})");
            assert!((cos(x) as f64 - (x as f64).cos()).abs() < 1e-7, "cos({x})");
        }
        for x in sweep(-3.2, 3.2, 200_000) {
            assert!(ulps(sin(x), (x as f64).sin()) <= 2, "sin({x})");
        }
    }

    #[test]
    fn exp_and_log_are_accurate() {
        for x in sweep(-87.0, 88.0, 1_000_000) {
            assert!(ulps(exp(x), (x as f64).exp()) <= 2, "exp({x})");
        }
        for x in (1..500_000u32).map(|i| f32::from_bits(i * 4271)) {
            assert!(ulps(log(x), (x as f64).ln()) <= 2, "log({x})");
        }
    }

    #[test]
    fn pow_error_grows_with_the_exponent() {
        for y in [2.4f32, 1.0 / 2.4, 0.5, 2.0, 3.0, -1.5, 7.0] {
            for x in sweep(0.001, 4.0, 100_000) {
                let size = (y as f64 * (x as f64).ln()).abs().max(1.0);
                let allowed = (8.0 * size) as u32;
                let got = ulps(pow(x, y), (x as f64).powf(y as f64));
                assert!(got <= allowed, "pow({x}, {y}) is {got} ulps out");
            }
        }
    }

    #[test]
    fn atan2_is_accurate() {
        for t in sweep(-3.14, 3.14, 500_000) {
            let (y, x) = ((t as f64).sin() as f32 * 3.0, (t as f64).cos() as f32 * 3.0);
            assert!(
                ulps(atan2(y, x), (y as f64).atan2(x as f64)) <= 4,
                "atan2({y}, {x})"
            );
        }
    }

    #[test]
    fn range_analysis_can_rely_on_these() {
        assert_eq!(exp(0.0), 1.0);
        for x in sweep(-1e5, 1e5, 1_000_000) {
            assert!(sin(x).abs() <= 1.0 && cos(x).abs() <= 1.0, "{x}");
        }
        for x in sweep(-150.0, 0.0, 500_000) {
            assert!((0.0..=1.0).contains(&exp(x)), "exp({x})");
        }
        for x in unit_floats() {
            assert!((0.0..=1.0).contains(&to_linear(x)), "to_linear({x})");
            assert!((0.0..=1.0).contains(&to_srgb(x)), "to_srgb({x})");
        }
    }

    #[test]
    fn special_values() {
        let nan = f32::NAN;
        let inf = f32::INFINITY;
        for (got, want) in [
            (sin(nan), nan),
            (sin(inf), nan),
            (cos(0.0), 1.0),
            (exp(nan), nan),
            (exp(inf), inf),
            (exp(-inf), 0.0),
            (exp(-200.0), 0.0),
            (log(0.0), -inf),
            (log(-1.0), nan),
            (log(inf), inf),
            (log(1.0), 0.0),
            (pow(-2.0, 3.0), -8.0),
            (pow(-2.0, 0.5), nan),
            (pow(nan, 0.0), 1.0),
            (pow(1.0, nan), 1.0),
            (pow(0.0, -1.0), inf),
            (atan2(0.0, 0.0), 0.0),
            (atan2(0.0, -1.0), PI),
            (atan2(1.0, 0.0), FRAC_PI_2),
            (atan2(nan, 1.0), nan),
        ] {
            assert!(
                got == want || (got.is_nan() && want.is_nan()),
                "{got} != {want}"
            );
        }
    }

    #[test]
    fn every_byte_survives_the_round_trip() {
        for byte in 0..=255u32 {
            let there = to_linear(byte as f32 / 255.0);
            let back = (to_srgb(there) * 255.0).round() as u32;
            assert_eq!(back, byte);
        }
    }
}
