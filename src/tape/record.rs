use std::cell::RefCell;
use std::collections::HashMap;

use super::optimise;
use super::range::{self, Range};
use super::{INPUTS, Op, Scalar, Step, Tape};

thread_local! {
    static RECORDING: RefCell<Option<Recorder>> = const { RefCell::new(None) };
}

struct Recorder {
    constant_slots: HashMap<u32, u32>,
    constants: Vec<(u32, f32)>,
    ranges: Vec<Range>,
    made_by: HashMap<u32, (Op, [Scalar; 3])>,
    recorded: HashMap<(Op, [u32; 3]), u32>,
    steps: Vec<Step>,
}

impl Recorder {
    fn new(inputs: [Range; INPUTS]) -> Recorder {
        Recorder {
            constant_slots: HashMap::new(),
            constants: Vec::new(),
            ranges: inputs.to_vec(),
            made_by: HashMap::new(),
            recorded: HashMap::new(),
            steps: Vec::new(),
        }
    }

    fn finish(mut self, outputs: [Scalar; 4]) -> Tape {
        let ranges = outputs.map(|out| self.range(out));
        let outputs = outputs.map(|out| self.register(out));
        let slots = self.ranges.len() as u32;
        Tape {
            inputs: std::array::from_fn(|i| self.ranges[i]),
            slots,
            constants: self.constants,
            steps: optimise::keep_needed(self.steps, outputs, slots),
            outputs,
            ranges,
        }
    }

    fn new_slot(&mut self, range: Range) -> u32 {
        self.ranges.push(range);
        self.ranges.len() as u32 - 1
    }

    fn register(&mut self, value: Scalar) -> u32 {
        match value {
            Scalar::Slot(slot) => slot,
            Scalar::Known(n) => match self.constant_slots.get(&n.to_bits()) {
                Some(&slot) => slot,
                None => {
                    let slot = self.new_slot(Range::of(n));
                    self.constant_slots.insert(n.to_bits(), slot);
                    self.constants.push((slot, n));
                    slot
                }
            },
        }
    }

    fn range(&self, value: Scalar) -> Range {
        match value {
            Scalar::Known(n) => Range::of(n),
            Scalar::Slot(slot) => self.ranges[slot as usize],
        }
    }

    fn record(&mut self, op: Op, args: [Scalar; 3]) -> Scalar {
        if let Some(shortcut) = self.simplify(op, args) {
            return shortcut;
        }
        let general = range::of_op(op, args.map(|arg| self.range(arg)));
        let range = match args {
            [x, y, _] if op == Op::Mul && x == y => range::square(self.range(x)),
            [x, y, _] if op == Op::Sub && self.is_floor_of(y, x) => {
                general.within(range::fraction(self.range(x)))
            }
            _ => general,
        };
        if let Some(n) = range.single() {
            return Scalar::Known(n);
        }
        let registers = args.map(|arg| self.register(arg));
        if let Some(&out) = self.recorded.get(&(op, registers)) {
            return Scalar::Slot(out);
        }
        let out = self.new_slot(range);
        self.recorded.insert((op, registers), out);
        self.made_by.insert(out, (op, args));
        self.steps.push(Step {
            op,
            args: registers,
            out,
        });
        Scalar::Slot(out)
    }

    fn simplify(&self, op: Op, [a, b, c]: [Scalar; 3]) -> Option<Scalar> {
        let [ra, rb, rc] = [a, b, c].map(|arg| self.range(arg));
        let one = Scalar::Known(1.0);
        let zero = |x: Scalar| matches!(x, Scalar::Known(n) if n.to_bits() == 0);
        let nothing = |r: Range| !r.nan && r.lo == 0.0 && r.hi == 0.0;
        match op {
            Op::IsNan if !ra.nan => Some(Scalar::Known(0.0)),
            Op::Select => match a {
                Scalar::Known(n) => Some(if n != 0.0 { b } else { c }),
                _ if !ra.nan && (ra.lo > 0.0 || ra.hi < 0.0) => Some(b),
                _ if !ra.nan && ra.lo == 0.0 && ra.hi == 0.0 => Some(c),
                _ => None,
            },
            Op::Mul if a == one => Some(b),
            Op::Mul if b == one => Some(a),
            Op::Mul if zero(a) && rb.is_finite() && rb.at_least_zero() => Some(a),
            Op::Mul if zero(b) && ra.is_finite() && ra.at_least_zero() => Some(b),
            Op::Add if nothing(rb) && !ra.minus_zero => Some(a),
            Op::Add if nothing(ra) && !rb.minus_zero => Some(b),
            Op::Sub if zero(b) => Some(a),
            Op::Div if b == one => Some(a),
            Op::Max if !ra.nan && !rb.nan && ra.lo > rb.hi => Some(a),
            Op::Max if !ra.nan && !rb.nan && rb.lo > ra.hi => Some(b),
            Op::Max if zero(b) && ra.at_least_zero() => Some(a),
            Op::Max if zero(a) && rb.at_least_zero() => Some(b),
            Op::Min if !ra.nan && !rb.nan && ra.hi < rb.lo => Some(a),
            Op::Min if !ra.nan && !rb.nan && rb.hi < ra.lo => Some(b),
            Op::Clamp if rb.nan || rc.nan => None,
            Op::Clamp if ra.lo >= rb.hi && ra.hi <= rc.lo => Some(a),
            Op::Clamp if ra.lo >= rb.hi && (a == c || self.fraction_of(a, c)) => Some(a),
            _ => None,
        }
    }

    fn is_floor_of(&self, value: Scalar, of: Scalar) -> bool {
        let Scalar::Slot(slot) = value else {
            return false;
        };
        matches!(self.made_by.get(&slot), Some(&(Op::Floor, [x, _, _])) if x == of)
    }

    fn fraction_of(&self, part: Scalar, whole: Scalar) -> bool {
        let Scalar::Slot(slot) = part else {
            return false;
        };
        let Some(&(Op::Mul, [x, y, _])) = self.made_by.get(&slot) else {
            return false;
        };
        let other = if x == whole {
            y
        } else if y == whole {
            x
        } else {
            return false;
        };
        let k = self.range(other);
        !k.nan && k.lo >= 0.0 && k.hi <= 1.0 && self.range(whole).at_least_zero()
    }
}

pub fn trace(op: Op, args: [Scalar; 3]) -> Scalar {
    RECORDING.with_borrow_mut(|recording| {
        let recorder = recording
            .as_mut()
            .expect("placeholders exist only while a tape is recording");
        recorder.record(op, args)
    })
}

impl Tape {
    pub fn record(
        inputs: [Range; INPUTS],
        body: impl FnOnce(&[Scalar]) -> Result<[Scalar; 4], String>,
    ) -> Result<Tape, String> {
        let placeholders: Vec<Scalar> = (0..INPUTS as u32).map(Scalar::Slot).collect();
        RECORDING.set(Some(Recorder::new(inputs)));
        let result = body(&placeholders);
        let recorder = RECORDING.take().expect("the recorder is still set");
        Ok(recorder.finish(result?))
    }

    pub fn specialise(&self, inputs: [Range; INPUTS]) -> Tape {
        let mut recorder = Recorder::new(inputs);
        let mut values: Vec<Scalar> = (0..self.slots).map(Scalar::Slot).collect();
        for &(slot, n) in &self.constants {
            values[slot as usize] = Scalar::Known(n);
        }
        for step in &self.steps {
            let args = step.args.map(|arg| values[arg as usize]);
            values[step.out as usize] = match args {
                [Scalar::Known(a), Scalar::Known(b), Scalar::Known(c)] => {
                    Scalar::Known(step.op.eval(a, b, c))
                }
                _ => recorder.record(step.op, args),
            };
        }
        recorder.finish(self.outputs.map(|out| values[out as usize]))
    }
}
