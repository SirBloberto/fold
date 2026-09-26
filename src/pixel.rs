use std::ops::{Add, Mul};

#[derive(Clone, Copy, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Colour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Colour {
    pub fn to_u32(self) -> u32 {
        let r = (self.r.clamp(0.0, 1.0) * 255.0).round() as u32;
        let g = (self.g.clamp(0.0, 1.0) * 255.0).round() as u32;
        let b = (self.b.clamp(0.0, 1.0) * 255.0).round() as u32;
        (r << 16) | (g << 8) | b
    }

    pub fn hex(rgb: u32) -> Colour {
        let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
        let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
        let b = (rgb & 0xFF) as f32 / 255.0;
        Colour { r, g, b }
    }
}

impl Add for Colour {
    type Output = Colour;
    fn add(self, o: Colour) -> Colour {
        Colour { r: self.r + o.r, g: self.g + o.g, b: self.b + o.b }
    }
}

impl Mul<f32> for Colour {
    type Output = Colour;
    fn mul(self, k: f32) -> Colour {
        Colour { r: self.r * k, g: self.g * k, b: self.b * k }
    }
}

pub fn mix(a: Colour, b: Colour, k: f32) -> Colour {
    a * (1.0 - k) + b * k
}

pub fn step(edge: f32, x: f32) -> f32 {
    if x < edge {
        0.0
    } else {
        1.0
    }
}

pub fn ripples(uv: Vec2, t: f32) -> Colour {
    let d = Vec2 { x: uv.x - 0.5, y: uv.y - 0.5 }.length();
    mix(Colour::hex(0x0b1d3a), Colour::hex(0x4fc3f7), 0.5 + 0.5 * (d * 60.0 - t * 4.0).sin())
}