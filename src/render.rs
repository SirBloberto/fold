mod output;
mod pool;

use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::tape::{INPUTS, Kernel, LANES, Lanes, Range, Scratch, TIME, Tape, X, Y};
use output::to_byte;
pub use output::to_u32;
use pool::Pool;

const TILE_ROWS: usize = 16;

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::default();
}

pub struct Renderer {
    canvas: Canvas,
    tiles: Arc<Vec<Mutex<Tile>>>,
    pixels: Vec<u32>,
    values: Vec<f32>,
    live: bool,
    stale: bool,
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
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub px: f32,
}

#[derive(Clone, Copy)]
struct Area {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
}

impl Renderer {
    pub fn new(tape: Arc<Tape>, canvas: Canvas, values: &[f32], live: bool) -> Renderer {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        let pool = Pool::new(cores);
        let areas = areas(canvas);
        let built: Arc<Vec<Mutex<Option<Tile>>>> =
            Arc::new(areas.iter().map(|_| Mutex::new(None)).collect());
        let mut inputs = tape.inputs.clone();
        if !live {
            for (range, &value) in inputs.iter_mut().zip(values).skip(INPUTS) {
                *range = Range::of(value);
            }
        }
        let next = AtomicUsize::new(0);
        pool.run({
            let built = built.clone();
            move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(&area) = areas.get(index) else {
                    break;
                };
                *lock(&built[index]) = Some(Tile::new(&tape, &inputs, area, canvas));
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
            canvas,
            tiles: Arc::new(tiles),
            pixels: vec![0; canvas.width * canvas.height],
            values: values.to_vec(),
            live,
            stale: false,
            pool,
        };
        renderer.show_fresh();
        renderer
    }

    pub fn set(&mut self, slot: usize, value: f32) {
        assert!(self.live, "only a live renderer takes new input values");
        if self.values[slot].to_bits() != value.to_bits() {
            self.values[slot] = value;
            self.stale = true;
        }
    }

    pub fn frame(&mut self, time: f32) -> &[u32] {
        self.values[TIME] = time;
        let stale = std::mem::take(&mut self.stale);
        let due: Vec<usize> = (0..self.tiles.len())
            .filter(|&tile| lock(&self.tiles[tile]).due(stale))
            .collect();
        if !due.is_empty() {
            let canvas = self.canvas;
            let tiles = self.tiles.clone();
            let values = Arc::new(self.values.clone());
            let next = AtomicUsize::new(0);
            self.pool.run(move || {
                SCRATCH.with_borrow_mut(|scratch| {
                    while let Some(&tile) = due.get(next.fetch_add(1, Ordering::Relaxed)) {
                        lock(&tiles[tile]).draw(scratch, &values, canvas);
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
                let start = (area.top + row) * self.canvas.width + area.left;
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
    fn new(tape: &Tape, inputs: &[Range], area: Area, canvas: Canvas) -> Tile {
        let tape = specialise(tape, inputs, area, canvas);
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

    fn due(&self, stale: bool) -> bool {
        self.kernel.as_ref().is_some_and(|kernel| {
            kernel.changes || !self.drawn || (stale && kernel.reacts)
        })
    }

    fn draw(&mut self, scratch: &mut Scratch, values: &[f32], canvas: Canvas) {
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
        let x = xs(area.left, canvas);
        let ys = (0..area.height).map(|row| (row, centre(area.top + row, canvas.height, canvas.px)));
        kernel.run(scratch, values, &x, ys, |row, [r, g, b, _]| {
            let line = &mut pixels[row * area.width..][..area.width];
            for (i, pixel) in line.iter_mut().enumerate() {
                *pixel = to_u32(r[i], g[i], b[i]);
            }
        });
        *drawn = true;
        *fresh = true;
    }
}

fn areas(canvas: Canvas) -> Vec<Area> {
    let mut areas = Vec::new();
    for top in (0..canvas.height).step_by(TILE_ROWS) {
        for left in (0..canvas.width).step_by(LANES) {
            areas.push(Area {
                left,
                top,
                width: LANES.min(canvas.width - left),
                height: TILE_ROWS.min(canvas.height - top),
            });
        }
    }
    areas
}

fn specialise(tape: &Tape, inputs: &[Range], area: Area, canvas: Canvas) -> Tape {
    let span = |first: usize, count: usize, length: usize| {
        Range::between(
            centre(first, length, canvas.px),
            centre(first + count - 1, length, canvas.px),
        )
    };
    let mut inputs = inputs.to_vec();
    inputs[X] = span(area.left, area.width, canvas.width);
    inputs[Y] = span(area.top, area.height, canvas.height);
    tape.specialise(&inputs)
}

fn constant_byte(range: Range) -> Option<u32> {
    let (lo, hi) = (to_byte(range.lo), to_byte(range.hi));
    (lo == hi && (!range.nan || lo == 0)).then_some(lo)
}

fn centre(index: usize, length: usize, px: f32) -> f32 {
    (index as f32 + 0.5 - length as f32 / 2.0) * px
}

fn xs(left: usize, canvas: Canvas) -> Lanes {
    std::array::from_fn(|i| centre(left + i, canvas.width, canvas.px))
}

#[cfg(test)]
pub fn trace(
    tape: &Tape,
    canvas: Canvas,
    values: &[f32],
    wanted: impl Fn(usize) -> bool,
    mut visit: impl FnMut(usize, usize, [f32; 4]),
) {
    let mut scratch = Scratch::default();
    for area in areas(canvas) {
        let kernel = Kernel::new(&specialise(tape, &tape.inputs, area, canvas));
        let rows = (0..area.height).filter(|&row| wanted(area.top + row));
        let ys = rows.map(|row| (row, centre(area.top + row, canvas.height, canvas.px)));
        kernel.run(&mut scratch, values, &xs(area.left, canvas), ys, |row, out| {
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
        let canvas = Canvas {
            width: 256,
            height: 256,
            px: 1.0,
        };
        let compiled = compile(&program, 1.0).unwrap();
        let values = compiled.defaults();
        Renderer::new(Arc::new(compiled.tape), canvas, &values, true)
    }

    fn due(renderer: &Renderer) -> usize {
        due_after_change(renderer, false)
    }

    fn due_after_change(renderer: &Renderer, stale: bool) -> usize {
        renderer
            .tiles
            .iter()
            .filter(|tile| lock(tile).due(stale))
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

    #[test]
    fn only_tiles_that_read_an_input_redraw_when_it_changes() {
        let mut renderer = renderer(
            "input glow: 0..1 = 0
draw FRAME |> fill(#101820)
draw circle(20) |> fill(#ffaa00 * glow)",
        );
        let dark = renderer.frame(0.0).to_vec();
        assert_eq!(due_after_change(&renderer, true), 8);
        renderer.set(TIME + 1, 1.0);
        let lit = renderer.frame(0.0).to_vec();
        assert_ne!(dark, lit);
        assert_eq!(dark[0], lit[0]);
        assert_eq!(due(&renderer), 0);
    }
}
