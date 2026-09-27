use crate::pixel::{Vec2, vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl Bounds {
    pub fn around(half: Vec2) -> Bounds {
        Bounds {
            min: half * -1.0,
            max: half,
        }
    }

    pub fn between(one: Vec2, two: Vec2) -> Bounds {
        Bounds {
            min: vec2(one.x.min(two.x), one.y.min(two.y)),
            max: vec2(one.x.max(two.x), one.y.max(two.y)),
        }
    }

    pub fn anchor(self, anc: Vec2) -> Vec2 {
        let mid = (self.min + self.max) * 0.5;
        let half = (self.max - self.min) * 0.5;
        vec2(mid.x + anc.x * half.x, mid.y + anc.y * half.y)
    }
}

pub fn circle(pt: Vec2, r: f32) -> f32 {
    pt.length() - r
}

pub fn rect(pt: Vec2, half: Vec2) -> f32 {
    let dx = pt.x.abs() - half.x;
    let dy = pt.y.abs() - half.y;
    let outside = vec2(dx.max(0.0), dy.max(0.0)).length();
    let inside = dx.max(dy).min(0.0);
    outside + inside
}

pub fn segment(pt: Vec2, from: Vec2, to: Vec2) -> f32 {
    let along = to - from;
    let rel = pt - from;
    let amt = (rel.dot(along) / along.dot(along)).clamp(0.0, 1.0);
    (rel - along * amt).length()
}
