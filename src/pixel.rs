use std::ops::{Add, Div, Mul, Sub};

use crate::tape::{self, Scalar};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: Scalar,
    pub y: Scalar,
}

pub fn vec2(x: impl Into<Scalar>, y: impl Into<Scalar>) -> Vec2 {
    Vec2 {
        x: x.into(),
        y: y.into(),
    }
}

impl Vec2 {
    pub fn length(self) -> Scalar {
        self.dot(self).sqrt()
    }

    pub fn dot(self, o: Vec2) -> Scalar {
        self.x * o.x + self.y * o.y
    }

    pub fn times(self, o: Vec2) -> Vec2 {
        vec2(self.x * o.x, self.y * o.y)
    }

    pub fn per(self, o: Vec2) -> Vec2 {
        vec2(self.x / o.x, self.y / o.y)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        vec2(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        vec2(self.x - o.x, self.y - o.y)
    }
}

impl<T: Into<Scalar> + Copy> Mul<T> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: T) -> Vec2 {
        vec2(self.x * k, self.y * k)
    }
}

impl<T: Into<Scalar> + Copy> Div<T> for Vec2 {
    type Output = Vec2;
    fn div(self, k: T) -> Vec2 {
        vec2(self.x / k, self.y / k)
    }
}

pub fn hash(pt: Vec2) -> Scalar {
    pt.x.hash(pt.y)
}

pub fn hash_bits(x: f32, y: f32) -> f32 {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: Scalar,
    pub g: Scalar,
    pub b: Scalar,
    pub a: Scalar,
}

impl Rgba {
    pub const CLEAR: Rgba = Rgba {
        r: Scalar::Known(0.0),
        g: Scalar::Known(0.0),
        b: Scalar::Known(0.0),
        a: Scalar::Known(0.0),
    };

    pub fn from_srgb(r: Scalar, g: Scalar, b: Scalar, a: Scalar) -> Rgba {
        Rgba {
            r: r.to_linear() * a,
            g: g.to_linear() * a,
            b: b.to_linear() * a,
            a,
        }
    }

    pub fn hex(rgba: u32) -> Rgba {
        let channel = |shift: u32| Scalar::from(((rgba >> shift) & 0xff) as f32 / 255.0);
        Rgba::from_srgb(channel(24), channel(16), channel(8), channel(0))
    }

    pub fn channels(self) -> [Scalar; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn to_srgb(self) -> [Scalar; 4] {
        let empty = self.a.at_most(0.0);
        [
            empty.select(0.0, (self.r / self.a).to_srgb()),
            empty.select(0.0, (self.g / self.a).to_srgb()),
            empty.select(0.0, (self.b / self.a).to_srgb()),
            empty.select(0.0, self.a),
        ]
    }

    pub fn valid(self) -> Rgba {
        let broken = self
            .r
            .is_nan()
            .max(self.g.is_nan())
            .max(self.b.is_nan())
            .max(self.a.is_nan());
        let a = self.a.clamp(0.0, 1.0);
        Rgba {
            r: broken.select(0.0, self.r.clamp(0.0, a)),
            g: broken.select(0.0, self.g.clamp(0.0, a)),
            b: broken.select(0.0, self.b.clamp(0.0, a)),
            a: broken.select(0.0, a),
        }
    }

    pub fn over(self, below: Rgba) -> Rgba {
        self + below * (1.0 - self.a)
    }
}

pub fn to_u32(r: f32, g: f32, b: f32) -> u32 {
    let byte = |light: f32| (to_srgb(tape::clamp(light, 0.0, 1.0)) * 255.0).round() as u32;
    (byte(r) << 16) | (byte(g) << 8) | byte(b)
}

pub fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn to_srgb(light: f32) -> f32 {
    if light <= 0.003_130_8 {
        light * 12.92
    } else {
        1.055 * light.powf(1.0 / 2.4) - 0.055
    }
}

impl Add for Rgba {
    type Output = Rgba;
    fn add(self, o: Rgba) -> Rgba {
        Rgba {
            r: self.r + o.r,
            g: self.g + o.g,
            b: self.b + o.b,
            a: self.a + o.a,
        }
    }
}

impl<T: Into<Scalar> + Copy> Mul<T> for Rgba {
    type Output = Rgba;
    fn mul(self, k: T) -> Rgba {
        Rgba {
            r: self.r * k,
            g: self.g * k,
            b: self.b * k,
            a: self.a * k,
        }
    }
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

    #[test]
    fn mid_grey_is_darker_in_linear_light() {
        let grey = Rgba::hex(0x808080ff).r.known().unwrap();
        assert!((grey - 0.2158).abs() < 1e-4, "{grey}");
    }

    #[test]
    fn colours_are_stored_premultiplied() {
        let [r, _, _, a] = Rgba::hex(0xffffff66).channels().map(|c| c.known().unwrap());
        assert!((r - 0.4).abs() < 1e-6);
        assert!((a - 0.4).abs() < 1e-6);
    }
}
