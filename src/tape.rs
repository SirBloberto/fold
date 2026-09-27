mod op;
mod optimise;
mod range;
mod record;
mod scalar;

pub use op::Op;
pub use range::Range;
pub use scalar::Scalar;

#[derive(Debug)]
pub struct Step {
    pub op: Op,
    pub args: [u32; 3],
    pub out: u32,
}

#[derive(Debug)]
pub struct Tape {
    pub slots: u32,
    pub pixel_inputs: u32,
    pub constants: Vec<(u32, f32)>,
    pub frame: Vec<Step>,
    pub pixel: Vec<Step>,
    pub outputs: [u32; 4],
}

impl Tape {
    pub fn slots(&self) -> Vec<f32> {
        let mut slots = vec![0.0; self.slots as usize];
        for &(slot, n) in &self.constants {
            slots[slot as usize] = n;
        }
        slots
    }

    pub fn start_frame(&self, slots: &mut [f32], inputs: &[f32]) {
        let first = self.pixel_inputs as usize;
        slots[first..first + inputs.len()].copy_from_slice(inputs);
        execute(&self.frame, slots);
    }

    pub fn run(&self, slots: &mut [f32], inputs: &[f32]) -> [f32; 4] {
        slots[..inputs.len()].copy_from_slice(inputs);
        execute(&self.pixel, slots);
        self.outputs.map(|out| slots[out as usize])
    }
}

fn execute(steps: &[Step], slots: &mut [f32]) {
    for step in steps {
        let [a, b, c] = step.args.map(|arg| slots[arg as usize]);
        slots[step.out as usize] = step.op.eval(a, b, c);
    }
}
