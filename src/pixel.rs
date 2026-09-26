use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
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

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2 {
            x: self.x - o.x,
            y: self.y - o.y,
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

pub fn mix(a: Rgba, b: Rgba, k: f32) -> Rgba {
    a * (1.0 - k) + b * k
}

pub fn step(edge: f32, x: f32) -> f32 {
    if x < edge { 0.0 } else { 1.0 }
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let k = ((x - a) / (b - a)).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

pub fn circle(p: Vec2, r: f32) -> f32 {
    p.length() - r
}

pub fn fill(d: f32, col: Rgba) -> Rgba {
    col * (1.0 - smoothstep(-0.5, 0.5, d))
}

pub fn glow(d: f32, col: Rgba, width: f32) -> Rgba {
    col * (-d.max(0.0) / width).exp()
}
