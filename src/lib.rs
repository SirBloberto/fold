mod eval;
mod maths;
mod png;
mod render;
mod syntax;
mod tape;

use eval::Declared;
use maths::Rgba;
pub use render::Area;
use render::{Canvas, Renderer};
use syntax::ast::Program;
use std::ops::Range;
use std::sync::Arc;
use tape::{Scalar, Tape};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Num(f32),
    Colour([f32; 4]),
}

#[derive(Clone, Debug)]
pub struct Input {
    pub name: String,
    pub range: Option<(f32, f32)>,
    pub default: Value,
}

pub struct Picture {
    program: Program,
    canvas: Canvas,
    inputs: Vec<Declared>,
    values: Vec<f32>,
    tape: Arc<Tape>,
    live: bool,
    rows: Option<Range<usize>>,
    renderer: Option<Renderer>,
}

pub fn load(source: &str) -> Result<Picture, String> {
    let (header, rest) = syntax::header::parse_header(source)?;
    let tokens = syntax::lexer::lex(rest, 2)?;
    let program = syntax::parser::parse(header, tokens)?;
    let canvas = Canvas {
        width: program.header.width as usize,
        height: program.header.height as usize,
        px: 1.0,
    };
    let compiled = eval::compile(&program, canvas.px)?;
    Ok(Picture {
        program,
        canvas,
        values: compiled.defaults(),
        inputs: compiled.inputs,
        tape: Arc::new(compiled.tape),
        live: false,
        rows: None,
        renderer: None,
    })
}

impl Picture {
    pub fn width(&self) -> usize {
        self.canvas.width
    }

    pub fn height(&self) -> usize {
        self.canvas.height
    }

    pub fn inputs(&self) -> Vec<Input> {
        let known = |channel: Scalar| channel.known().unwrap_or(0.0);
        let describe = |input: &Declared| Input {
            name: input.name.clone(),
            range: input.range,
            default: match input.default[..] {
                [n] => Value::Num(n),
                [r, g, b, a] => {
                    let linear = Rgba {
                        r: r.into(),
                        g: g.into(),
                        b: b.into(),
                        a: a.into(),
                    };
                    Value::Colour(linear.to_srgb().map(known))
                }
                _ => unreachable!("an input is one num or four channels"),
            },
        };
        self.inputs.iter().map(describe).collect()
    }

    pub fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        let Some(input) = self.inputs.iter().find(|input| input.name == name) else {
            return Err(format!("there is no input named `{name}`"));
        };
        let channels = match (input.range, value) {
            (Some((lo, hi)), Value::Num(n)) => vec![if n.is_nan() { lo } else { n.clamp(lo, hi) + 0.0 }],
            (None, Value::Colour([r, g, b, a])) => {
                let colour = Rgba::from_srgb(r.into(), g.into(), b.into(), a.into()).valid();
                colour.channels().map(|c| c.known().unwrap_or(0.0)).to_vec()
            }
            (Some(_), Value::Colour(_)) => return Err(format!("`{name}` is a num")),
            (None, Value::Num(_)) => return Err(format!("`{name}` is a colour")),
        };
        let slots = input.slot..input.slot + channels.len();
        if !self.live && self.values[slots.clone()] != channels[..] {
            self.live = true;
            self.renderer = None;
        }
        for (slot, channel) in slots.zip(channels) {
            self.values[slot] = channel;
            if let Some(renderer) = self.renderer.as_mut().filter(|_| self.live) {
                renderer.set(slot, channel);
            }
        }
        Ok(())
    }

    pub fn resize(&mut self, width: usize) {
        let header = &self.program.header;
        let px = header.width as f32 / width.max(1) as f32;
        let height = ((header.height as f32 / px).round() as usize).max(1);
        if (width, height) == (self.canvas.width, self.canvas.height) {
            return;
        }
        self.canvas = Canvas { width, height, px };
        let compiled = eval::compile(&self.program, px);
        self.tape = Arc::new(compiled.expect("a picture that loads compiles at every size").tape);
        self.renderer = None;
    }

    pub fn band(&mut self, rows: Range<usize>) {
        if self.rows.as_ref() != Some(&rows) {
            self.rows = Some(rows);
            self.renderer = None;
        }
    }

    pub fn frame(&mut self, time: f32) -> &[u32] {
        self.renderer().frame(time)
    }

    pub fn changed(&self) -> &[Area] {
        self.renderer.as_ref().map_or(&[], Renderer::changed)
    }

    pub fn pixels(&self) -> &[u32] {
        self.renderer.as_ref().map_or(&[], Renderer::pixels)
    }

    pub fn png(&mut self, time: f32) -> Vec<u8> {
        let canvas = self.canvas;
        png::encode(canvas.width, canvas.height, self.frame(time))
    }

    fn renderer(&mut self) -> &mut Renderer {
        let (tape, canvas) = (&self.tape, self.canvas);
        let rows = self.rows.clone().unwrap_or(0..canvas.height);
        self.renderer.get_or_insert_with(|| {
            Renderer::new(tape.clone(), canvas, &self.values, self.live, rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(body: &str) -> Picture {
        load(&format!("~fold v1 64x64
{body}")).unwrap()
    }

    #[test]
    fn inputs_describe_themselves() {
        let picture = picture("input level: 0..10 = 4
input tint = #ff8000
draw tint * (level / 10)");
        let inputs = picture.inputs();
        assert_eq!(inputs[0].name, "level");
        assert_eq!(inputs[0].range, Some((0.0, 10.0)));
        assert_eq!(inputs[0].default, Value::Num(4.0));
        assert_eq!(inputs[1].range, None);
        let Value::Colour([r, g, b, a]) = inputs[1].default else {
            panic!("tint is a colour");
        };
        assert!((r - 1.0).abs() < 1e-5 && (g - 128.0 / 255.0).abs() < 1e-5);
        assert!(b.abs() < 1e-5 && (a - 1.0).abs() < 1e-5);
    }

    #[test]
    fn nums_are_clamped_to_their_range() {
        let mut picture = picture("input level: 0..1 = 0
draw #ffffff * level");
        assert_eq!(picture.frame(0.0)[0], 0x000000);
        picture.set("level", Value::Num(1.0)).unwrap();
        assert_eq!(picture.frame(0.0)[0], 0xffffff);
        picture.set("level", Value::Num(-3.0)).unwrap();
        assert_eq!(picture.frame(0.0)[0], 0x000000);
        picture.set("level", Value::Num(7.0)).unwrap();
        assert_eq!(picture.frame(0.0)[0], 0xffffff);
        picture.set("level", Value::Num(f32::NAN)).unwrap();
        assert_eq!(picture.frame(0.0)[0], 0x000000);
    }

    #[test]
    fn colours_arrive_as_srgb() {
        let mut picture = picture("input tint = #000000
draw tint");
        let orange = [1.0, 128.0 / 255.0, 0.0, 1.0];
        picture.set("tint", Value::Colour(orange)).unwrap();
        let mut literal = self::picture("draw #ff8000");
        assert_eq!(picture.frame(0.0), literal.frame(0.0));
    }

    #[test]
    fn inputs_go_live_on_the_first_change() {
        let mut picture = picture("input level: 0..1 = 0.5\ndraw #ffffff * level");
        picture.frame(0.0);
        picture.set("level", Value::Num(0.5)).unwrap();
        assert!(!picture.live);
        picture.set("level", Value::Num(0.25)).unwrap();
        assert!(picture.live);
        let mut literal = self::picture("draw #ffffff * 0.25");
        assert_eq!(picture.frame(0.0), literal.frame(0.0));
    }

    #[test]
    fn a_band_draws_the_same_rows() {
        let body = "draw circle(20) |> fill(#ffaa00 * (0.5 + 0.5 * sin(TIME)))";
        let mut whole = picture(body);
        let mut band = picture(body);
        band.band(16..40);
        let (whole, band) = (whole.frame(1.0).to_vec(), band.frame(1.0).to_vec());
        assert_eq!(whole[16 * 64..40 * 64], band[16 * 64..40 * 64]);
    }

    #[test]
    fn inputs_survive_a_resize() {
        let mut picture = picture("input level: 0..1 = 0
draw #ffffff * level");
        picture.set("level", Value::Num(1.0)).unwrap();
        picture.resize(16);
        assert_eq!((picture.width(), picture.height()), (16, 16));
        assert_eq!(picture.frame(0.0)[0], 0xffffff);
    }

    #[test]
    fn setting_the_wrong_input_fails() {
        let mut picture = picture("input level: 0..1 = 0
input tint = #000000
draw tint * level");
        assert_eq!(
            picture.set("glow", Value::Num(1.0)),
            Err("there is no input named `glow`".into())
        );
        assert_eq!(
            picture.set("level", Value::Colour([1.0; 4])),
            Err("`level` is a num".into())
        );
        assert_eq!(
            picture.set("tint", Value::Num(1.0)),
            Err("`tint` is a colour".into())
        );
    }
}
