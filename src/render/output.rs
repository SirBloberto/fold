use std::sync::LazyLock;

struct Table {
    thresholds: [f32; 256],
    first: u32,
    start: Vec<u8>,
}

static TABLE: LazyLock<Table> = LazyLock::new(|| {
    let mut thresholds = [f32::INFINITY; 256];
    for (k, threshold) in thresholds.iter_mut().take(255).enumerate() {
        *threshold = to_linear((k as f64 + 0.5) / 255.0) as f32;
    }
    let first = bucket(thresholds[0]);
    let start = (first..=bucket(1.0))
        .map(|b| {
            let low = f32::from_bits(b << 16);
            thresholds.iter().take_while(|&&t| t <= low).count() as u8
        })
        .collect();
    Table {
        thresholds,
        first,
        start,
    }
});

fn bucket(light: f32) -> u32 {
    light.to_bits() >> 16
}

pub fn to_u32(r: f32, g: f32, b: f32) -> u32 {
    let table = &*TABLE;
    (byte(table, r) << 16) | (byte(table, g) << 8) | byte(table, b)
}

pub fn to_byte(light: f32) -> u32 {
    byte(&TABLE, light)
}

#[inline(always)]
fn byte(table: &Table, light: f32) -> u32 {
    let light = if light > 0.0 { light.min(1.0) } else { 0.0 };
    let index = bucket(light).saturating_sub(table.first) as usize;
    let start = table.start[index.min(table.start.len() - 1)];
    u32::from(start) + u32::from(light >= table.thresholds[usize::from(start)])
}

fn to_linear(srgb: f64) -> f64 {
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maths::Rgba;

    fn to_srgb(light: f64) -> f64 {
        if light <= 0.003_130_8 {
            light * 12.92
        } else {
            1.055 * light.powf(1.0 / 2.4) - 0.055
        }
    }

    #[test]
    fn each_bucket_holds_at_most_one_threshold() {
        let table = &*TABLE;
        let buckets = table.start.len() as u32;
        for index in 0..buckets {
            let low = match index {
                0 => 0.0,
                _ => f32::from_bits((table.first + index) << 16),
            };
            let high = f32::from_bits((table.first + index + 1) << 16);
            let inside = table.thresholds.iter().filter(|&&t| low < t && t < high);
            assert!(inside.count() <= 1, "bucket {index}");
        }
    }

    #[test]
    fn every_threshold_is_exact() {
        for (k, &threshold) in TABLE.thresholds.iter().take(255).enumerate() {
            assert_eq!(to_byte(threshold), k as u32 + 1);
            assert_eq!(to_byte(threshold.next_down()), k as u32);
        }
    }

    #[test]
    fn every_byte_survives_the_round_trip() {
        for value in 0..=255u32 {
            let light = Rgba::hex(value << 24 | 0xff).r.known().unwrap();
            assert_eq!(to_byte(light), value);
        }
    }

    #[test]
    fn bytes_follow_the_srgb_curve() {
        for bits in (0..=1.0f32.to_bits()).step_by(4099) {
            let light = f32::from_bits(bits);
            let exact = to_srgb(light as f64) * 255.0;
            if (exact.fract() - 0.5).abs() < 1e-4 {
                continue;
            }
            assert_eq!(to_byte(light), exact.round() as u32, "{light}");
        }
    }

    #[test]
    fn odd_values_are_clamped() {
        for (light, want) in [
            (f32::NAN, 0),
            (-f32::NAN, 0),
            (f32::NEG_INFINITY, 0),
            (-1.0, 0),
            (-0.0, 0),
            (0.0, 0),
            (f32::MIN_POSITIVE / 2.0, 0),
            (1.0, 255),
            (2.0, 255),
            (f32::INFINITY, 255),
        ] {
            assert_eq!(to_byte(light), want, "{light}");
        }
    }

    #[test]
    fn channels_are_packed_as_rgb() {
        assert_eq!(to_u32(1.0, 0.0, 1.0), 0xff00ff);
    }
}
