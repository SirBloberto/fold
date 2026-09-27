use std::ops::{Add, Div, Mul, Sub};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

pub fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

impl Vec2 {
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2 {
            x: self.x + o.x,
            y: self.y + o.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2 {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f32) -> Vec2 {
        Vec2 {
            x: self.x * k,
            y: self.y * k,
        }
    }
}

impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, k: f32) -> Vec2 {
        Vec2 {
            x: self.x / k,
            y: self.y / k,
        }
    }
}

impl Mul for Vec2 {
    type Output = Vec2;
    fn mul(self, o: Vec2) -> Vec2 {
        Vec2 {
            x: self.x * o.x,
            y: self.y * o.y,
        }
    }
}

impl Div for Vec2 {
    type Output = Vec2;
    fn div(self, o: Vec2) -> Vec2 {
        Vec2 {
            x: self.x / o.x,
            y: self.y / o.y,
        }
    }
}

pub fn hash(pt: Vec2) -> f32 {
    let x = (pt.x + 0.0).to_bits();
    let y = (pt.y + 0.0).to_bits();
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
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub fn from_srgb(r: f32, g: f32, b: f32, a: f32) -> Rgba {
        Rgba {
            r: to_linear(r) * a,
            g: to_linear(g) * a,
            b: to_linear(b) * a,
            a,
        }
    }

    pub fn hex(rgba: u32) -> Rgba {
        let channel = |shift: u32| ((rgba >> shift) & 0xff) as f32 / 255.0;
        Rgba::from_srgb(channel(24), channel(16), channel(8), channel(0))
    }

    pub fn to_srgb(self) -> [f32; 4] {
        if self.a <= 0.0 {
            return [0.0; 4];
        }
        [
            to_srgb(self.r / self.a),
            to_srgb(self.g / self.a),
            to_srgb(self.b / self.a),
            self.a,
        ]
    }

    pub fn to_u32(self) -> u32 {
        let byte = |light: f32| (to_srgb(light.clamp(0.0, 1.0)) * 255.0).round() as u32;
        (byte(self.r) << 16) | (byte(self.g) << 8) | byte(self.b)
    }
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

impl Mul<f32> for Rgba {
    type Output = Rgba;
    fn mul(self, k: f32) -> Rgba {
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
        let grey = Rgba::hex(0x808080ff);
        assert!((grey.r - 0.2158).abs() < 1e-4, "{}", grey.r);
    }

    #[test]
    fn colours_are_stored_premultiplied() {
        let faint = Rgba::hex(0xffffff66);
        assert!((faint.r - 0.4).abs() < 1e-6);
        assert!((faint.a - 0.4).abs() < 1e-6);
    }
}
