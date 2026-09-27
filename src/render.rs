use crate::tape::{self, Tape};

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
    let mut slots = tape.slots();
    for (i, out) in pixels.iter_mut().enumerate() {
        let x = (i % width) as f32 + 0.5 - width as f32 / 2.0;
        let y = (first_row + i / width) as f32 + 0.5 - height as f32 / 2.0;
        let [r, g, b, _] = tape.run(&mut slots, &[x, y, time]);
        *out = to_u32(r, g, b);
    }
}

pub fn to_u32(r: f32, g: f32, b: f32) -> u32 {
    let byte = |light: f32| (tape::to_srgb(tape::clamp(light, 0.0, 1.0)) * 255.0).round() as u32;
    (byte(r) << 16) | (byte(g) << 8) | byte(b)
}
