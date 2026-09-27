use std::ops::{Add, Div, Mul, Sub};

use crate::tape::Scalar;

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
