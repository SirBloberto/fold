mod output;
mod pool;

use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::tape::{Kernel, LANES, Lanes, Range, Scratch, Tape, X, Y};
use output::to_byte;
pub use output::to_u32;
use pool::Pool;

const TILE_ROWS: usize = 16;

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::default();
}

pub struct Renderer {
    width: usize,
    height: usize,
    tiles: Arc<Vec<Mutex<Tile>>>,
    pixels: Vec<u32>,
    pool: Pool,
}

struct Tile {
    area: Area,
    kernel: Option<Kernel>,
    pixels: Vec<u32>,
    drawn: bool,
    fresh: bool,
}

#[derive(Clone, Copy)]
struct Area {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
}

impl Renderer {
    pub fn new(tape: Tape, width: usize, height: usize) -> Renderer {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        let pool = Pool::new(cores);
        let areas = areas(width, height);
        let built: Arc<Vec<Mutex<Option<Tile>>>> =
            Arc::new(areas.iter().map(|_| Mutex::new(None)).collect());
        let tape = Arc::new(tape);
        let next = AtomicUsize::new(0);
        pool.run({
            let built = built.clone();
            move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(&area) = areas.get(index) else {
                    break;
                };
                *lock(&built[index]) = Some(Tile::new(&tape, area, width, height));
            }
        });
        let tiles = Arc::into_inner(built)
            .expect("the pool has finished with the tiles")
            .into_iter()
            .map(|tile| {
                Mutex::new(
                    tile.into_inner()
                        .expect("no thread panics while building")
                        .expect("every tile is built"),
                )
            })
            .collect();
        let mut renderer = Renderer {
            width,
            height,
            tiles: Arc::new(tiles),
            pixels: vec![0; width * height],
            pool,
        };
        renderer.show_fresh();
        renderer
    }

    pub fn frame(&mut self, time: f32) -> &[u32] {
        let due: Vec<usize> = (0..self.tiles.len())
            .filter(|&tile| lock(&self.tiles[tile]).due())
            .collect();
        if !due.is_empty() {
            let (width, height) = (self.width, self.height);
            let tiles = self.tiles.clone();
            let next = AtomicUsize::new(0);
            self.pool.run(move || {
                SCRATCH.with_borrow_mut(|scratch| {
                    while let Some(&tile) = due.get(next.fetch_add(1, Ordering::Relaxed)) {
                        lock(&tiles[tile]).draw(scratch, time, width, height);
                    }
                })
            });
            self.show_fresh();
        }
        &self.pixels
    }

    fn show_fresh(&mut self) {
        for tile in self.tiles.iter() {
            let mut tile = lock(tile);
            if !tile.fresh {
                continue;
            }
            let area = tile.area;
            for (row, line) in tile.pixels.chunks(area.width).enumerate() {
                let start = (area.top + row) * self.width + area.left;
                self.pixels[start..start + area.width].copy_from_slice(line);
            }
            tile.fresh = false;
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().expect("no thread panics while drawing")
}

impl Tile {
    fn new(tape: &Tape, area: Area, width: usize, height: usize) -> Tile {
        let tape = specialise(tape, area, width, height);
        let [r, g, b, _] = tape.ranges;
        match [r, g, b].map(constant_byte) {
            [Some(r), Some(g), Some(b)] => Tile {
                area,
                kernel: None,
                pixels: vec![(r << 16) | (g << 8) | b; area.width * area.height],
                drawn: true,
                fresh: true,
            },
            _ => Tile {
                area,
                kernel: Some(Kernel::new(&tape)),
                pixels: vec![0; area.width * area.height],
                drawn: false,
                fresh: false,
            },
        }
    }

    fn due(&self) -> bool {
        self.kernel
            .as_ref()
            .is_some_and(|kernel| kernel.changes || !self.drawn)
    }

    fn draw(&mut self, scratch: &mut Scratch, time: f32, width: usize, height: usize) {
        let Tile {
            area,
            kernel: Some(kernel),
            pixels,
            drawn,
            fresh,
        } = self
        else {
            return;
        };
        let x = xs(area.left, width);
        let ys = (0..area.height).map(|row| (row, centre(area.top + row, height)));
        kernel.run(scratch, time, &x, ys, |row, [r, g, b, _]| {
            let line = &mut pixels[row * area.width..][..area.width];
            for (i, pixel) in line.iter_mut().enumerate() {
                *pixel = to_u32(r[i], g[i], b[i]);
            }
        });
        *drawn = true;
        *fresh = true;
    }
}

fn areas(width: usize, height: usize) -> Vec<Area> {
    let mut areas = Vec::new();
    for top in (0..height).step_by(TILE_ROWS) {
        for left in (0..width).step_by(LANES) {
            areas.push(Area {
                left,
                top,
                width: LANES.min(width - left),
                height: TILE_ROWS.min(height - top),
            });
        }
    }
    areas
}

fn specialise(tape: &Tape, area: Area, width: usize, height: usize) -> Tape {
    let span = |first: usize, count: usize, length: usize| {
        Range::between(centre(first, length), centre(first + count - 1, length))
    };
    let mut inputs = tape.inputs;
    inputs[X] = span(area.left, area.width, width);
    inputs[Y] = span(area.top, area.height, height);
    tape.specialise(inputs)
}

fn constant_byte(range: Range) -> Option<u32> {
    let (lo, hi) = (to_byte(range.lo), to_byte(range.hi));
    (lo == hi && (!range.nan || lo == 0)).then_some(lo)
}

fn centre(index: usize, length: usize) -> f32 {
    index as f32 + 0.5 - length as f32 / 2.0
}

fn xs(left: usize, width: usize) -> Lanes {
    std::array::from_fn(|i| centre(left + i, width))
}

#[cfg(test)]
pub fn trace(
    tape: &Tape,
    width: usize,
    height: usize,
    time: f32,
    wanted: impl Fn(usize) -> bool,
    mut visit: impl FnMut(usize, usize, [f32; 4]),
) {
    let mut scratch = Scratch::default();
    for area in areas(width, height) {
        let kernel = Kernel::new(&specialise(tape, area, width, height));
        let rows = (0..area.height).filter(|&row| wanted(area.top + row));
        let ys = rows.map(|row| (row, centre(area.top + row, height)));
        kernel.run(&mut scratch, time, &xs(area.left, width), ys, |row, out| {
            for i in 0..area.width {
                visit(area.left + i, area.top + row, out.map(|lanes| lanes[i]));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::compile;
    use crate::syntax::{header::parse_header, lexer::lex, parser::parse};

    fn renderer(body: &str) -> Renderer {
        let src = format!("~fold v1 256x256\n{body}");
        let (header, rest) = parse_header(&src).unwrap();
        let program = parse(header, lex(rest, 2).unwrap()).unwrap();
        Renderer::new(compile(&program, 1.0).unwrap(), 256, 256)
    }

    fn due(renderer: &Renderer) -> usize {
        renderer
            .tiles
            .iter()
            .filter(|tile| lock(tile).due())
            .count()
    }

    #[test]
    fn tiles_skip_shapes_they_cannot_see() {
        let renderer = renderer("draw FRAME |> fill(#101820)\ndraw circle(20) |> fill(#ffaa00)");
        let fills = renderer
            .tiles
            .iter()
            .filter(|tile| lock(tile).kernel.is_none());
        assert_eq!(fills.count(), renderer.tiles.len() - 8);
        assert_eq!(renderer.pixels[0], 0x101820);
    }

    #[test]
    fn still_pictures_are_drawn_once() {
        let mut renderer = renderer("draw circle(40) |> glow(#ffaa00, 30)");
        let first = renderer.frame(0.0).to_vec();
        assert_eq!(due(&renderer), 0);
        assert_eq!(renderer.frame(5.0), first);
    }

    #[test]
    fn only_changing_tiles_are_drawn_again() {
        let mut renderer = renderer(
            "draw circle(40) |> glow(#ffaa00, 30)\ndraw circle(8) |> at(sin(TIME) * 10, 0) |> fill(#ffffff)",
        );
        renderer.frame(0.0);
        let changing = due(&renderer);
        assert!(changing > 0 && changing < renderer.tiles.len() / 4);
    }

    #[test]
    fn fast_noise_leaves_tiles_it_cannot_reach() {
        let mut renderer =
            renderer("draw circle(20) |> fill(#ffffff * noise(vec2(TIME * 1000000, 0)))");
        renderer.frame(0.0);
        assert!(due(&renderer) <= 8);
    }

    #[test]
    fn layers_that_are_zero_leave_the_tile() {
        let mut renderer = renderer(
            "draw #ffffff * (0.5 + POS.x / 600)\ndraw circle(20) |> fill(#ffffff * (noise(vec2(TIME, 0)) - 0.5))",
        );
        renderer.frame(0.0);
        assert!(due(&renderer) <= 8);
    }
}
