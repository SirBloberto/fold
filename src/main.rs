mod eval;
mod pixel;
mod syntax;

use minifb::{Key, Window, WindowOptions};
use pixel::Vec2;
use std::time::Instant;
use syntax::ast::Program;

fn main() {
    let path = std::env::args().nth(1).unwrap_or("examples/sun.fld".into());
    let program = match load(&path) {
        Ok(program) => program,
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    };

    let width = program.header.width as usize;
    let height = program.header.height as usize;
    let mut buffer = vec![0; width * height];
    let mut window = Window::new("Fold", width, height, WindowOptions::default())
        .expect("could not open window");
    window.set_target_fps(60);

    let start = Instant::now();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let time = start.elapsed().as_secs_f32();

        if let Err(e) = render(&program, &mut buffer, width, height, time) {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }

        window
            .update_with_buffer(&buffer, width, height)
            .expect("could not update window");
    }
}

fn load(path: &str) -> Result<Program, String> {
    let src = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (header, rest) = syntax::header::parse_header(&src)?;
    let tokens = syntax::lexer::lex(rest, 2)?;
    syntax::parser::parse(header, tokens)
}

fn render(
    program: &Program,
    buffer: &mut [u32],
    width: usize,
    height: usize,
    time: f32,
) -> Result<(), String> {
    let size = Vec2 {
        x: width as f32,
        y: height as f32,
    };

    for y in 0..height {
        for x in 0..width {
            let pixel = eval::Pixel {
                p: Vec2 {
                    x: x as f32 + 0.5 - size.x / 2.0,
                    y: y as f32 + 0.5 - size.y / 2.0,
                },
                uv: Vec2 {
                    x: (x as f32 + 0.5) / size.x,
                    y: (y as f32 + 0.5) / size.y,
                },
                time,
                px: 1.0,
                size,
            };
            buffer[y * width + x] = eval::run(program, &pixel)?.to_u32();
        }
    }
    Ok(())
}
