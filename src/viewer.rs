use std::time::{Duration, Instant, SystemTime};

use crate::eval;
use crate::render::Renderer;
use crate::syntax;
use crate::syntax::ast::Program;

const FRAME_BUDGET: Duration = Duration::from_micros(16_667);

pub struct Stats {
    since: Instant,
    frames: u32,
    total: Duration,
    worst: Duration,
}

impl Stats {
    pub fn new() -> Stats {
        Stats {
            since: Instant::now(),
            frames: 0,
            total: Duration::ZERO,
            worst: Duration::ZERO,
        }
    }

    pub fn record(&mut self, render: Duration) {
        self.frames += 1;
        self.total += render;
        self.worst = self.worst.max(render);

        let elapsed = self.since.elapsed();
        if elapsed < Duration::from_secs(1) {
            return;
        }

        let fps = self.frames as f32 / elapsed.as_secs_f32();
        let average = self.total / self.frames;
        let budget = average.as_secs_f32() / FRAME_BUDGET.as_secs_f32() * 100.0;
        println!(
            "{fps:>3.0} fps · render {:>5.1} ms avg, {:>5.1} ms worst · {budget:>3.0}% of a 60 fps frame",
            average.as_secs_f32() * 1000.0,
            self.worst.as_secs_f32() * 1000.0,
        );

        *self = Stats::new();
    }
}

pub struct Live {
    pub path: String,
    pub program: Program,
    pub renderer: Renderer,
    modified: Option<SystemTime>,
}

impl Live {
    pub fn open(path: String) -> Result<Live, String> {
        let (program, renderer) = load(&path)?;
        Ok(Live {
            modified: modified(&path),
            path,
            program,
            renderer,
        })
    }

    pub fn refresh(&mut self) {
        let now = modified(&self.path);
        if now == self.modified {
            return;
        }
        self.modified = now;

        match load(&self.path) {
            Ok((program, _)) if program.header != self.program.header => {
                eprintln!("{}: restart to change the header", self.path);
            }
            Ok((program, renderer)) => {
                self.program = program;
                self.renderer = renderer;
                println!("{}: reloaded", self.path);
            }
            Err(e) => eprintln!("{}: {e}", self.path),
        }
    }
}

fn load(path: &str) -> Result<(Program, Renderer), String> {
    let src = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (header, rest) = syntax::header::parse_header(&src)?;
    let tokens = syntax::lexer::lex(rest, 2)?;
    let program = syntax::parser::parse(header, tokens)?;
    let tape = eval::compile(&program, 1.0)?;
    let (width, height) = (program.header.width, program.header.height);
    let renderer = Renderer::new(tape, width as usize, height as usize);
    Ok((program, renderer))
}

fn modified(path: &str) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}
