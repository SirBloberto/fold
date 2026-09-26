use std::time::SystemTime;

use crate::syntax;
use crate::syntax::ast::Program;

pub struct Live {
    pub path: String,
    pub program: Program,
    pub broken: bool,
    modified: Option<SystemTime>,
}

impl Live {
    pub fn open(path: String) -> Result<Live, String> {
        let program = load(&path)?;
        Ok(Live {
            modified: modified(&path),
            path,
            program,
            broken: false,
        })
    }

    pub fn refresh(&mut self) {
        let now = modified(&self.path);
        if now == self.modified {
            return;
        }
        self.modified = now;

        match load(&self.path) {
            Ok(program) if program.header != self.program.header => {
                eprintln!("{}: restart to change the header", self.path);
            }
            Ok(program) => {
                self.program = program;
                self.broken = false;
                println!("{}: reloaded", self.path);
            }
            Err(e) => eprintln!("{}: {e}", self.path),
        }
    }
}

fn load(path: &str) -> Result<Program, String> {
    let src = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (header, rest) = syntax::header::parse_header(&src)?;
    let tokens = syntax::lexer::lex(rest, 2)?;
    syntax::parser::parse(header, tokens)
}

fn modified(path: &str) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}
