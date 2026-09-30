use super::{Vec2, vec2};
use crate::tape::Scalar;

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reach {
    Everywhere,
    Box { min: Vec2, max: Vec2 },
    Ring { centre: Vec2, inner: Scalar, outer: Scalar },
}

impl Reach {
    pub fn around(half: Vec2) -> Reach {
        let half = vec2(half.x.max(0.0), half.y.max(0.0));
        Reach::Box {
            min: half * -1.0,
            max: half,
        }
    }

    pub fn distance(self, pt: Vec2) -> Option<Scalar> {
        match self {
            Reach::Everywhere => None,
            Reach::Box { min, max } => {
                let x = (min.x - pt.x).max(pt.x - max.x).max(0.0);
                let y = (min.y - pt.y).max(pt.y - max.y).max(0.0);
                Some(vec2(x, y).length())
            }
            Reach::Ring {
                centre,
                inner,
                outer,
            } => {
                let d = (pt - centre).length();
                Some((inner - d).max(d - outer))
            }
        }
    }

    fn boxed(self) -> Reach {
        match self {
            Reach::Ring { centre, outer, .. } => Reach::Box {
                min: centre - vec2(outer, outer),
                max: centre + vec2(outer, outer),
            },
            other => other,
        }
    }

    pub fn moved(self, by: Vec2) -> Reach {
        match self {
            Reach::Everywhere => Reach::Everywhere,
            Reach::Box { min, max } => Reach::Box {
                min: min + by,
                max: max + by,
            },
            Reach::Ring {
                centre,
                inner,
                outer,
            } => Reach::Ring {
                centre: centre + by,
                inner,
                outer,
            },
        }
    }

    pub fn spun(self) -> Reach {
        match self {
            Reach::Everywhere => Reach::Everywhere,
            Reach::Box { min, max } => {
                let far_x = min.x.abs().max(max.x.abs());
                let far_y = min.y.abs().max(max.y.abs());
                Reach::Ring {
                    centre: vec2(0.0, 0.0),
                    inner: self.distance(vec2(0.0, 0.0)).unwrap_or(Scalar::from(0.0)),
                    outer: vec2(far_x, far_y).length(),
                }
            }
            Reach::Ring {
                centre,
                inner,
                outer,
            } => {
                let away = centre.length();
                Reach::Ring {
                    centre: vec2(0.0, 0.0),
                    inner: (inner - away).max(away - outer).max(0.0),
                    outer: away + outer,
                }
            }
        }
    }

    pub fn mirrored(self) -> Reach {
        match self.boxed() {
            Reach::Box { min, max } => Reach::Box {
                min: vec2(min.x.min(-max.x), min.y),
                max: vec2(max.x.max(-min.x), max.y),
            },
            other => other,
        }
    }

    pub fn joined(self, other: Reach) -> Reach {
        match (self.boxed(), other.boxed()) {
            (Reach::Box { min, max }, Reach::Box { min: low, max: high }) => Reach::Box {
                min: vec2(min.x.min(low.x), min.y.min(low.y)),
                max: vec2(max.x.max(high.x), max.y.max(high.y)),
            },
            _ => Reach::Everywhere,
        }
    }

    pub fn widened(self, by: Scalar) -> Reach {
        let by = by.max(0.0);
        match self {
            Reach::Everywhere => Reach::Everywhere,
            Reach::Box { min, max } => Reach::Box {
                min: min - vec2(by, by),
                max: max + vec2(by, by),
            },
            Reach::Ring {
                centre,
                inner,
                outer,
            } => Reach::Ring {
                centre,
                inner: inner - by,
                outer: outer + by,
            },
        }
    }

    pub fn scaled(self, by: Scalar) -> Reach {
        match (self, by.known()) {
            (Reach::Box { min, max }, Some(k)) if k > 0.0 => Reach::Box {
                min: min * k,
                max: max * k,
            },
            (
                Reach::Ring {
                    centre,
                    inner,
                    outer,
                },
                Some(k),
            ) if k > 0.0 => Reach::Ring {
                centre: centre * k,
                inner: inner * k,
                outer: outer * k,
            },
            _ => Reach::Everywhere,
        }
    }
}

pub fn circle(pt: Vec2, r: Scalar) -> Scalar {
    pt.length() - r
}

pub fn rect(pt: Vec2, half: Vec2) -> Scalar {
    let dx = pt.x.abs() - half.x;
    let dy = pt.y.abs() - half.y;
    let outside = vec2(dx.max(0.0), dy.max(0.0)).length();
    let inside = dx.max(dy).min(0.0);
    outside + inside
}

pub fn segment(pt: Vec2, from: Vec2, to: Vec2) -> Scalar {
    let along = to - from;
    let rel = pt - from;
    let amt = (rel.dot(along) / along.dot(along)).clamp(0.0, 1.0);
    (rel - along * amt).length()
}
