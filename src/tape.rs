use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::pixel;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scalar {
    Known(f32),
    Slot(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Sqrt,
    Abs,
    Floor,
    Sin,
    Cos,
    Exp,
    Pow,
    Atan2,
    Min,
    Max,
    Clamp,
    ToLinear,
    ToSrgb,
    Hash,
    IsNan,
    AtMost,
    Select,
}

impl Op {
    pub fn eval(self, a: f32, b: f32, c: f32) -> f32 {
        match self {
            Op::Add => a + b,
            Op::Sub => a - b,
            Op::Mul => a * b,
            Op::Div => a / b,
            Op::Neg => -a,
            Op::Sqrt => a.sqrt(),
            Op::Abs => a.abs(),
            Op::Floor => a.floor(),
            Op::Sin => a.sin(),
            Op::Cos => a.cos(),
            Op::Exp => a.exp(),
            Op::Pow => a.powf(b),
            Op::Atan2 => a.atan2(b),
            Op::Min => a.min(b),
            Op::Max => a.max(b),
            Op::Clamp => clamp(a, b, c),
            Op::ToLinear => pixel::to_linear(a),
            Op::ToSrgb => pixel::to_srgb(a),
            Op::Hash => pixel::hash_bits(a, b),
            Op::IsNan => flag(a.is_nan()),
            Op::AtMost => flag(a <= b),
            Op::Select => {
                if a != 0.0 {
                    b
                } else {
                    c
                }
            }
        }
    }
}

pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

fn flag(yes: bool) -> f32 {
    if yes { 1.0 } else { 0.0 }
}

impl Scalar {
    pub fn known(self) -> Option<f32> {
        match self {
            Scalar::Known(n) => Some(n),
            Scalar::Slot(_) => None,
        }
    }

    fn apply(op: Op, args: [Scalar; 3]) -> Scalar {
        if let [Scalar::Known(a), Scalar::Known(b), Scalar::Known(c)] = args {
            return Scalar::Known(op.eval(a, b, c));
        }
        RECORDING.with_borrow_mut(|recording| {
            let recorder = recording
                .as_mut()
                .expect("placeholders exist only while a tape is recording");
            recorder.push(op, args)
        })
    }

    fn unary(self, op: Op) -> Scalar {
        Scalar::apply(op, [self, ZERO, ZERO])
    }

    fn binary(self, op: Op, other: impl Into<Scalar>) -> Scalar {
        Scalar::apply(op, [self, other.into(), ZERO])
    }

    pub fn sqrt(self) -> Scalar {
        self.unary(Op::Sqrt)
    }

    pub fn abs(self) -> Scalar {
        self.unary(Op::Abs)
    }

    pub fn floor(self) -> Scalar {
        self.unary(Op::Floor)
    }

    pub fn sin(self) -> Scalar {
        self.unary(Op::Sin)
    }

    pub fn cos(self) -> Scalar {
        self.unary(Op::Cos)
    }

    pub fn exp(self) -> Scalar {
        self.unary(Op::Exp)
    }

    pub fn to_linear(self) -> Scalar {
        self.unary(Op::ToLinear)
    }

    pub fn to_srgb(self) -> Scalar {
        self.unary(Op::ToSrgb)
    }

    pub fn is_nan(self) -> Scalar {
        self.unary(Op::IsNan)
    }

    pub fn powf(self, by: impl Into<Scalar>) -> Scalar {
        self.binary(Op::Pow, by)
    }

    pub fn atan2(self, x: impl Into<Scalar>) -> Scalar {
        self.binary(Op::Atan2, x)
    }

    pub fn min(self, other: impl Into<Scalar>) -> Scalar {
        self.binary(Op::Min, other)
    }

    pub fn max(self, other: impl Into<Scalar>) -> Scalar {
        self.binary(Op::Max, other)
    }

    pub fn at_most(self, other: impl Into<Scalar>) -> Scalar {
        self.binary(Op::AtMost, other)
    }

    pub fn hash(self, y: Scalar) -> Scalar {
        self.binary(Op::Hash, y)
    }

    pub fn clamp(self, lo: impl Into<Scalar>, hi: impl Into<Scalar>) -> Scalar {
        Scalar::apply(Op::Clamp, [self, lo.into(), hi.into()])
    }

    pub fn select(self, yes: impl Into<Scalar>, no: impl Into<Scalar>) -> Scalar {
        Scalar::apply(Op::Select, [self, yes.into(), no.into()])
    }
}

const ZERO: Scalar = Scalar::Known(0.0);

impl From<f32> for Scalar {
    fn from(n: f32) -> Scalar {
        Scalar::Known(n)
    }
}

impl fmt::Display for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Scalar::Known(n) => write!(f, "{n}"),
            Scalar::Slot(_) => write!(f, "a per-pixel value"),
        }
    }
}

impl Neg for Scalar {
    type Output = Scalar;
    fn neg(self) -> Scalar {
        self.unary(Op::Neg)
    }
}

macro_rules! operator {
    ($trait:ident, $method:ident, $op:expr) => {
        impl<T: Into<Scalar>> $trait<T> for Scalar {
            type Output = Scalar;
            fn $method(self, other: T) -> Scalar {
                self.binary($op, other)
            }
        }

        impl $trait<Scalar> for f32 {
            type Output = Scalar;
            fn $method(self, other: Scalar) -> Scalar {
                Scalar::from(self).binary($op, other)
            }
        }
    };
}

operator!(Add, add, Op::Add);
operator!(Sub, sub, Op::Sub);
operator!(Mul, mul, Op::Mul);
operator!(Div, div, Op::Div);

thread_local! {
    static RECORDING: RefCell<Option<Recorder>> = const { RefCell::new(None) };
}

struct Recorder {
    next: u32,
    constants: HashMap<u32, u32>,
    tape: Tape,
}

impl Recorder {
    fn register(&mut self, value: Scalar) -> u32 {
        match value {
            Scalar::Slot(slot) => slot,
            Scalar::Known(n) => *self.constants.entry(n.to_bits()).or_insert_with(|| {
                let slot = self.next;
                self.next += 1;
                self.tape.constants.push((slot, n));
                slot
            }),
        }
    }

    fn push(&mut self, op: Op, args: [Scalar; 3]) -> Scalar {
        let args = args.map(|arg| self.register(arg));
        let out = self.next;
        self.next += 1;
        self.tape.steps.push(Step { op, args, out });
        Scalar::Slot(out)
    }
}

#[derive(Debug)]
pub struct Step {
    pub op: Op,
    pub args: [u32; 3],
    pub out: u32,
}

#[derive(Debug, Default)]
pub struct Tape {
    pub slots: u32,
    pub constants: Vec<(u32, f32)>,
    pub steps: Vec<Step>,
    pub outputs: [u32; 4],
}

impl Tape {
    pub fn record(
        inputs: u32,
        body: impl FnOnce(&[Scalar]) -> Result<[Scalar; 4], String>,
    ) -> Result<Tape, String> {
        RECORDING.set(Some(Recorder {
            next: inputs,
            constants: HashMap::new(),
            tape: Tape::default(),
        }));
        let placeholders: Vec<Scalar> = (0..inputs).map(Scalar::Slot).collect();
        let result = body(&placeholders);
        let mut recorder = RECORDING.take().expect("the recorder is still set");
        let outputs = result?.map(|out| recorder.register(out));
        recorder.tape.outputs = outputs;
        recorder.tape.slots = recorder.next;
        Ok(recorder.tape)
    }

    pub fn slots(&self) -> Vec<f32> {
        let mut slots = vec![0.0; self.slots as usize];
        for &(slot, n) in &self.constants {
            slots[slot as usize] = n;
        }
        slots
    }

    pub fn run(&self, slots: &mut [f32], inputs: &[f32]) -> [f32; 4] {
        slots[..inputs.len()].copy_from_slice(inputs);
        for step in &self.steps {
            let [a, b, c] = step.args.map(|arg| slots[arg as usize]);
            slots[step.out as usize] = step.op.eval(a, b, c);
        }
        self.outputs.map(|out| slots[out as usize])
    }
}
