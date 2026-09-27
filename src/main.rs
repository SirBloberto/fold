mod eval;
mod maths;
mod render;
mod syntax;
mod tape;
mod viewer;

use minifb::{Key, Window, WindowOptions};
use std::time::Instant;

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
        render::frame(&live.tape, &mut buffer, width, height, time);
        stats.record(frame.elapsed());

        window
            .update_with_buffer(&buffer, width, height)
            .expect("could not update window");
    }
}
