mod batch;
mod float;
mod kernel;
mod op;
mod optimise;
mod range;
mod record;
mod scalar;

pub use batch::{LANES, Lanes};
pub use kernel::{Kernel, Scratch};
pub use op::Op;
pub use range::Range;
pub use record::declare;
pub use scalar::Scalar;

pub const X: usize = 0;
pub const Y: usize = 1;
pub const TIME: usize = 2;
pub const INPUTS: usize = 3;

#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub op: Op,
    pub args: [u32; 3],
    pub out: u32,
}

#[derive(Debug)]
pub struct Tape {
    pub inputs: Vec<Range>,
    pub slots: u32,
    pub constants: Vec<(u32, f32)>,
    pub steps: Vec<Step>,
    pub outputs: [u32; 4],
    pub ranges: [Range; 4],
}

fn execute(steps: &[Step], slots: &mut [f32]) {
    for step in steps {
        let [a, b, c] = step.args.map(|arg| slots[arg as usize]);
        slots[step.out as usize] = step.op.eval(a, b, c);
    }
}
