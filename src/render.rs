use std::sync::LazyLock;

use crate::tape::{LANES, Tape};

static THRESHOLDS: LazyLock<[f32; 255]> =
    LazyLock::new(|| std::array::from_fn(|k| to_linear((k as f64 + 0.5) / 255.0) as f32));

pub fn frame(tape: &Tape, buffer: &mut [u32], width: usize, height: usize, time: f32) {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let rows_per_band = height.div_ceil(threads);

    std::thread::scope(|s| {
        for (band, pixels) in buffer.chunks_mut(rows_per_band * width).enumerate() {
            let first_row = band * rows_per_band;
            s.spawn(move || rows(tape, pixels, first_row, width, height, time));
        }
    });
}

fn rows(tape: &Tape, pixels: &mut [u32], first_row: usize, width: usize, height: usize, time: f32) {
    let mut values = tape.slots();
    tape.start_frame(&mut values, &[time]);
    let mut lanes = tape.batch.prepare(&values);
    for (row, line) in pixels.chunks_mut(width).enumerate() {
        let y = (first_row + row) as f32 + 0.5 - height as f32 / 2.0;
        for (chunk, out) in line.chunks_mut(LANES).enumerate() {
            let first = chunk * LANES;
            let x = std::array::from_fn(|i| (first + i) as f32 + 0.5 - width as f32 / 2.0);
            let [r, g, b, _] = tape.batch.run(&mut lanes, &[x, [y; LANES]]);
            for (i, pixel) in out.iter_mut().enumerate() {
                *pixel = to_u32(r[i], g[i], b[i]);
            }
        }
    }
}

pub fn to_u32(r: f32, g: f32, b: f32) -> u32 {
    let thresholds = &*THRESHOLDS;
    let byte = |light: f32| {
        let mut reached = 0;
        for step in [128, 64, 32, 16, 8, 4, 2, 1] {
            reached += step * usize::from(light >= thresholds[reached + step - 1]);
        }
        reached as u32
    };
    (byte(r) << 16) | (byte(g) << 8) | byte(b)
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

    fn byte(light: f32) -> u32 {
        to_u32(light, 0.0, 0.0) >> 16
    }

    fn to_srgb(light: f64) -> f64 {
        if light <= 0.003_130_8 {
            light * 12.92
        } else {
            1.055 * light.powf(1.0 / 2.4) - 0.055
        }
    }

    #[test]
    fn every_byte_survives_the_round_trip() {
        for value in 0..=255u32 {
            let light = Rgba::hex(value << 24 | 0xff).r.known().unwrap();
            assert_eq!(byte(light), value);
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
            assert_eq!(byte(light), exact.round() as u32, "{light}");
        }
    }

    #[test]
    fn odd_values_are_clamped() {
        for (light, want) in [
            (f32::NAN, 0),
            (f32::NEG_INFINITY, 0),
            (-1.0, 0),
            (-0.0, 0),
            (0.0, 0),
            (1.0, 255),
            (2.0, 255),
            (f32::INFINITY, 255),
        ] {
            assert_eq!(byte(light), want, "{light}");
        }
    }
}
