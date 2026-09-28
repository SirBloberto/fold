use super::{Op, Step};

pub const LANES: usize = 64;

pub type Lanes = [f32; LANES];

#[derive(Debug)]
pub struct Batch {
    pub steps: Vec<Step>,
    pub rows: usize,
    pub broadcast: Vec<(u32, u32)>,
    pub outputs: [u32; 4],
}

impl Batch {
    pub fn new(pixel: &[Step], pixel_inputs: u32, outputs: [u32; 4], slots: u32) -> Batch {
        let slots = slots as usize;
        let mut made_here = vec![false; slots];
        let mut last_use = vec![0; slots];
        for (i, step) in pixel.iter().enumerate() {
            made_here[step.out as usize] = true;
            for arg in step.args {
                last_use[arg as usize] = i;
            }
        }
        for out in outputs {
            last_use[out as usize] = usize::MAX;
        }

        let mut row_of: Vec<Option<u32>> = vec![None; slots];
        let mut rows = 0;
        for slot in 0..pixel_inputs as usize {
            row_of[slot] = Some(rows);
            rows += 1;
        }
        let mut broadcast = Vec::new();
        let read = pixel.iter().flat_map(|step| step.args).chain(outputs);
        for slot in read {
            let slot = slot as usize;
            if row_of[slot].is_none() && !made_here[slot] {
                row_of[slot] = Some(rows);
                broadcast.push((slot as u32, rows));
                rows += 1;
            }
        }

        let mut free = Vec::new();
        let mut steps = Vec::new();
        for (i, step) in pixel.iter().enumerate() {
            let args = step
                .args
                .map(|arg| row_of[arg as usize].expect("an argument has a row"));
            for (n, arg) in step.args.iter().enumerate() {
                let arg = *arg as usize;
                let first = !step.args[..n].contains(&step.args[n]);
                if made_here[arg] && last_use[arg] == i && first {
                    free.push(row_of[arg].expect("an argument has a row"));
                }
            }
            let out = free.pop().unwrap_or_else(|| {
                rows += 1;
                rows - 1
            });
            row_of[step.out as usize] = Some(out);
            steps.push(Step {
                op: step.op,
                args,
                out,
            });
        }

        Batch {
            steps,
            rows: rows as usize,
            broadcast,
            outputs: outputs.map(|out| row_of[out as usize].expect("an output has a row")),
        }
    }

    pub fn prepare(&self, values: &[f32]) -> Vec<Lanes> {
        let mut rows = vec![[0.0; LANES]; self.rows];
        for &(slot, row) in &self.broadcast {
            rows[row as usize] = [values[slot as usize]; LANES];
        }
        rows
    }

    pub fn run<'a>(&self, rows: &'a mut [Lanes], inputs: &[Lanes]) -> [&'a Lanes; 4] {
        rows[..inputs.len()].copy_from_slice(inputs);
        for step in &self.steps {
            let [a, b, c] = step.args.map(|arg| arg as usize);
            let result = apply(step.op, &rows[a], &rows[b], &rows[c]);
            rows[step.out as usize] = result;
        }
        let rows: &'a [Lanes] = rows;
        self.outputs.map(|out| &rows[out as usize])
    }
}

macro_rules! by_op {
    ($op:expr, $a:expr, $b:expr, $c:expr, $($name:ident)*) => {
        match $op {
            $(Op::$name => each($a, $b, $c, |a, b, c| Op::$name.eval(a, b, c)),)*
        }
    };
}

fn apply(op: Op, a: &Lanes, b: &Lanes, c: &Lanes) -> Lanes {
    by_op!(op, a, b, c,
        Add Sub Mul Div Neg Sqrt Abs Floor Sin Cos Exp Pow Atan2
        Min Max Clamp ToLinear ToSrgb Hash IsNan AtMost Select)
}

#[inline(always)]
fn each(a: &Lanes, b: &Lanes, c: &Lanes, f: impl Fn(f32, f32, f32) -> f32) -> Lanes {
    let mut out = [0.0; LANES];
    for i in 0..LANES {
        out[i] = f(a[i], b[i], c[i]);
    }
    out
}
