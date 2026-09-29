mod eval;
mod maths;
mod png;
mod render;
mod syntax;
mod tape;

use render::Renderer;

pub struct Picture {
    width: usize,
    height: usize,
    renderer: Renderer,
}

pub fn load(source: &str) -> Result<Picture, String> {
    let (header, rest) = syntax::header::parse_header(source)?;
    let tokens = syntax::lexer::lex(rest, 2)?;
    let program = syntax::parser::parse(header, tokens)?;
    let tape = eval::compile(&program, 1.0)?;
    let (width, height) = (
        program.header.width as usize,
        program.header.height as usize,
    );
    Ok(Picture {
        width,
        height,
        renderer: Renderer::new(tape, width, height),
    })
}

impl Picture {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn frame(&mut self, time: f32) -> &[u32] {
        self.renderer.frame(time)
    }

    pub fn png(&mut self, time: f32) -> Vec<u8> {
        let (width, height) = (self.width, self.height);
        png::encode(width, height, self.renderer.frame(time))
    }
}
