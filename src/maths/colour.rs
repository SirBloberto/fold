use std::ops::{Add, Mul};

use crate::tape::Scalar;

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

    pub fn lit(self, light: Rgba) -> Rgba {
        Rgba {
            r: self.r * light.r,
            g: self.g * light.g,
            b: self.b * light.b,
            a: self.a,
        }
    }

    pub fn over(self, below: Rgba) -> Rgba {
        self + below * (1.0 - self.a)
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
    fn mid_grey_is_darker_in_linear_light() {
        let grey = Rgba::hex(0x808080ff).r.known().unwrap();
        assert!((grey - 0.2158).abs() < 1e-4, "{grey}");
    }

    #[test]
    fn light_multiplies_each_channel_and_keeps_the_surface_alpha() {
        let surface = Rgba::hex(0x808080cc);
        let light = Rgba::hex(0xff8000ff) * 0.5;
        let [r, g, b, a] = surface.lit(light).channels().map(|c| c.known().unwrap());
        let [sr, sg, _, sa] = surface.channels().map(|c| c.known().unwrap());
        let [lr, lg, _, _] = light.channels().map(|c| c.known().unwrap());
        assert_eq!((r, g, b, a), (sr * lr, sg * lg, 0.0, sa));
    }

    #[test]
    fn colours_are_stored_premultiplied() {
        let [r, _, _, a] = Rgba::hex(0xffffff66).channels().map(|c| c.known().unwrap());
        assert!((r - 0.4).abs() < 1e-6);
        assert!((a - 0.4).abs() < 1e-6);
    }
}
