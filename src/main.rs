mod pixel;
mod syntax;
use pixel::{Colour, Vec2, circle, fill, glow};

use minifb::{Key, Window, WindowOptions};
use std::time::Instant;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

fn main() {
    let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut window = Window::new("Test", WIDTH, HEIGHT, WindowOptions::default())
        .expect("could not open window");

    window.set_target_fps(60);
    let start = Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let t = start.elapsed().as_secs_f32();

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let u = x as f32 / WIDTH as f32;
                let v = y as f32 / HEIGHT as f32;

                let uv = Vec2 { x: u, y: v };
                let p = Vec2 {
                    x: x as f32 - WIDTH as f32 / 2.0,
                    y: y as f32 - HEIGHT as f32 / 2.0,
                };
                buffer[y * WIDTH + x] = scene(p, uv, t).to_u32();
            }
        }

        window
            .update_with_buffer(&buffer, WIDTH, HEIGHT)
            .expect("could not update window");
    }
}

fn scene(p: Vec2, _uv: Vec2, t: f32) -> Colour {
    let orange = Colour::hex(0xffaa00);
    let d = circle(p, 100.0);
    let w = 20.0 + (t * 2.0).sin() * 6.0;
    fill(d, orange) + glow(d, orange, w)
}
