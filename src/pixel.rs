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
}

impl Rgba {
    pub fn to_u32(self) -> u32 {
        let r = (self.r.clamp(0.0, 1.0) * 255.0).round() as u32;
        let g = (self.g.clamp(0.0, 1.0) * 255.0).round() as u32;
        let b = (self.b.clamp(0.0, 1.0) * 255.0).round() as u32;
        (r << 16) | (g << 8) | b
    }

    pub fn hex(rgb: u32) -> Rgba {
        let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
        let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
        let b = (rgb & 0xFF) as f32 / 255.0;
        Rgba { r, g, b }
    }
}

impl Add for Rgba {
    type Output = Rgba;
    fn add(self, o: Rgba) -> Rgba {
        Rgba {
            r: self.r + o.r,
            g: self.g + o.g,
            b: self.b + o.b,
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
        }
    }
}
