mod viewer;

use minifb::{Key, Window, WindowOptions};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("render") => render(&args[1..]),
        Some(path) => view(path),
        None => view("examples/sun.fld"),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn render(args: &[String]) -> Result<(), String> {
    let usage = "usage: fold render <file.fld> <out.png> [--time seconds] [--width pixels]";
    let [input, output, rest @ ..] = args else {
        return Err(usage.into());
    };
    let (mut time, mut width) = (0.0, None);
    for pair in rest.chunks(2) {
        match pair {
            [flag, value] if flag == "--time" => time = value.parse().map_err(|_| usage)?,
            [flag, value] if flag == "--width" => {
                width = Some(value.parse().ok().filter(|&w| w > 0).ok_or(usage)?)
            }
            _ => return Err(usage.into()),
        }
    }
    let source = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
    let mut picture = fold::load(&source).map_err(|e| format!("{input}: {e}"))?;
    if let Some(width) = width {
        picture.resize(width);
    }
    std::fs::write(output, picture.png(time)).map_err(|e| format!("{output}: {e}"))
}

fn view(path: &str) -> Result<(), String> {
    let mut live = viewer::Live::open(path.into())?;
    let (width, height) = (live.picture.width(), live.picture.height());
    let mut window = Window::new("Fold", width, height, WindowOptions::default())
        .map_err(|e| format!("could not open a window: {e}"))?;
    window.set_target_fps(60);

    let start = Instant::now();
    let mut stats = viewer::Stats::new();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        live.refresh();

        let time = start.elapsed().as_secs_f32();
        let frame = Instant::now();
        let pixels = live.picture.frame(time);
        stats.record(frame.elapsed());

        window
            .update_with_buffer(pixels, width, height)
            .map_err(|e| format!("could not update the window: {e}"))?;
    }
    Ok(())
}
