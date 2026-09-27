mod eval;
mod pixel;
mod shape;
mod syntax;
mod tape;
mod viewer;

use minifb::{Key, Window, WindowOptions};
use std::time::Instant;
use tape::Tape;

fn main() {
    let path = std::env::args().nth(1).unwrap_or("examples/sun.fld".into());
    let mut live = match viewer::Live::open(path) {
        Ok(live) => live,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let width = live.program.header.width as usize;
    let height = live.program.header.height as usize;
    let mut buffer = vec![0; width * height];
    let mut window = Window::new("Fold", width, height, WindowOptions::default())
        .expect("could not open window");
    window.set_target_fps(60);

    let start = Instant::now();
    let mut stats = viewer::Stats::new();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        live.refresh();

        let time = start.elapsed().as_secs_f32();
        let frame = Instant::now();
        render(&live.tape, &mut buffer, width, height, time);
        stats.record(frame.elapsed());

        window
            .update_with_buffer(&buffer, width, height)
            .expect("could not update window");
    }
}

fn render(tape: &Tape, buffer: &mut [u32], width: usize, height: usize, time: f32) {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let rows_per_band = height.div_ceil(threads);

    std::thread::scope(|s| {
        for (band, pixels) in buffer.chunks_mut(rows_per_band * width).enumerate() {
            let first_row = band * rows_per_band;
            s.spawn(move || render_rows(tape, pixels, first_row, width, height, time));
        }
    });
}

fn render_rows(
    tape: &Tape,
    pixels: &mut [u32],
    first_row: usize,
    width: usize,
    height: usize,
    time: f32,
) {
    let mut slots = tape.slots();
    for (i, out) in pixels.iter_mut().enumerate() {
        let x = (i % width) as f32 + 0.5 - width as f32 / 2.0;
        let y = (first_row + i / width) as f32 + 0.5 - height as f32 / 2.0;
        let [r, g, b, _] = tape.run(&mut slots, &[x, y, time, 1.0]);
        *out = pixel::to_u32(r, g, b);
    }
}
