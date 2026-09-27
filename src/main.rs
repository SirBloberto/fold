mod eval;
mod pixel;
mod shape;
mod syntax;
mod viewer;

use minifb::{Key, Window, WindowOptions};
use pixel::Vec2;
use std::time::Instant;
use syntax::ast::Program;

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

        if !live.broken {
            let time = start.elapsed().as_secs_f32();
            let frame = Instant::now();
            if let Err(e) = render(&live.program, &mut buffer, width, height, time) {
                eprintln!("{}: {e}", live.path);
                live.broken = true;
            }
            stats.record(frame.elapsed());
        }

        window
            .update_with_buffer(&buffer, width, height)
            .expect("could not update window");
    }
}

fn render(
    program: &Program,
    buffer: &mut [u32],
    width: usize,
    height: usize,
    time: f32,
) -> Result<(), String> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let rows_per_band = height.div_ceil(threads);

    std::thread::scope(|s| {
        let bands: Vec<_> = buffer
            .chunks_mut(rows_per_band * width)
            .enumerate()
            .map(|(band, pixels)| {
                let first_row = band * rows_per_band;
                s.spawn(move || render_rows(program, pixels, first_row, width, height, time))
            })
            .collect();

        bands
            .into_iter()
            .map(|band| band.join().expect("render thread panicked"))
            .collect()
    })
}

fn render_rows(
    program: &Program,
    pixels: &mut [u32],
    first_row: usize,
    width: usize,
    height: usize,
    time: f32,
) -> Result<(), String> {
    let size = Vec2 {
        x: width as f32,
        y: height as f32,
    };
    let prelude = eval::Prelude::load(size)?;

    for (i, out) in pixels.iter_mut().enumerate() {
        let x = (i % width) as f32 + 0.5;
        let y = (first_row + i / width) as f32 + 0.5;
        let pixel = eval::Pixel {
            pos: Vec2 {
                x: x - size.x / 2.0,
                y: y - size.y / 2.0,
            },
            time,
            px: 1.0,
            size,
        };
        *out = eval::run(program, &prelude, pixel)?.to_u32();
    }
    Ok(())
}
