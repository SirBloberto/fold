use super::{Op, Step, X};

pub const LANES: usize = 64;

pub type Lanes = [f32; LANES];

#[derive(Debug)]
pub struct Batch {
    pub columns: Vec<Step>,
    pub pixel: Vec<Step>,
    pub rows: usize,
    pub fixed: Vec<(u32, u32)>,
    pub by_row: Vec<(u32, u32)>,
    pub outputs: [u32; 4],
}

impl Batch {
    pub fn new(columns: &[Step], pixel: &[Step], outputs: [u32; 4], by_row: &[bool]) -> Batch {
        let slots = by_row.len();
        let steps: Vec<&Step> = columns.iter().chain(pixel).collect();
        let mut made_here = vec![false; slots];
        let mut last_use = vec![0; slots];
        for (i, step) in steps.iter().enumerate() {
            made_here[step.out as usize] = true;
            for arg in step.args {
                last_use[arg as usize] = i;
            }
        }
        for step in columns {
            if last_use[step.out as usize] >= columns.len() {
                last_use[step.out as usize] = usize::MAX;
            }
        }
        for out in outputs {
            last_use[out as usize] = usize::MAX;
        }

        let mut row_of: Vec<Option<u32>> = vec![None; slots];
        row_of[X] = Some(0);
        let mut rows = 1;
        let (mut fixed, mut changing) = (Vec::new(), Vec::new());
        let read = steps.iter().flat_map(|step| step.args).chain(outputs);
        for slot in read {
            let slot = slot as usize;
            if row_of[slot].is_none() && !made_here[slot] {
                row_of[slot] = Some(rows);
                let list = if by_row[slot] {
                    &mut changing
                } else {
                    &mut fixed
                };
                list.push((slot as u32, rows));
                rows += 1;
            }
        }

        let mut free = Vec::new();
        let mut placed = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            let args = step
                .args
                .map(|arg| row_of[arg as usize].expect("an argument has a row"));
            let mut released = Vec::new();
            for (n, arg) in step.args.iter().enumerate() {
                let arg = *arg as usize;
                let first = !step.args[..n].contains(&step.args[n]);
                if made_here[arg] && last_use[arg] == i && first {
                    released.push(row_of[arg].expect("an argument has a row"));
                }
            }
            let out = free.pop().unwrap_or_else(|| {
                rows += 1;
                rows - 1
            });
            free.extend(released);
            row_of[step.out as usize] = Some(out);
            placed.push(Step {
                op: step.op,
                args,
                out,
            });
        }

        let pixel = placed.split_off(columns.len());
        Batch {
            columns: placed,
            pixel,
            rows: rows as usize,
            fixed,
            by_row: changing,
            outputs: outputs.map(|out| row_of[out as usize].expect("an output has a row")),
        }
    }

    pub fn start(&self, rows: &mut Vec<Lanes>, values: &[f32], x: &Lanes) {
        if rows.len() < self.rows {
            rows.resize(self.rows, [0.0; LANES]);
        }
        rows[0] = *x;
        for &(slot, row) in &self.fixed {
            rows[row as usize] = [values[slot as usize]; LANES];
        }
        execute(&self.columns, rows);
    }

    pub fn run<'a>(&self, rows: &'a mut [Lanes], values: &[f32]) -> [&'a Lanes; 4] {
        for &(slot, row) in &self.by_row {
            rows[row as usize] = [values[slot as usize]; LANES];
        }
        execute(&self.pixel, rows);
        let rows: &'a [Lanes] = rows;
        self.outputs.map(|out| &rows[out as usize])
    }
}

fn execute(steps: &[Step], rows: &mut [Lanes]) {
    for step in steps {
        let out = step.out as usize;
        let (before, rest) = rows.split_at_mut(out);
        let (target, after) = rest
            .split_first_mut()
            .expect("the output row exists");
        let read = |arg: u32| -> &Lanes {
            let arg = arg as usize;
            if arg < out {
                &before[arg]
            } else {
                &after[arg - out - 1]
            }
        };
        let [a, b, c] = step.args.map(read);
        apply(step.op, a, b, c, target);
    }
}

macro_rules! by_op {
    ($op:expr, $a:expr, $b:expr, $c:expr, $out:expr, $($name:ident)*) => {
        match $op {
            $(Op::$name => each($a, $b, $c, $out, |a, b, c| Op::$name.eval(a, b, c)),)*
        }
    };
}

pub fn apply(op: Op, a: &Lanes, b: &Lanes, c: &Lanes, out: &mut Lanes) {
    by_op!(op, a, b, c, out,
        Add Sub Mul Div Neg Sqrt Abs Floor Sin Cos Exp Pow Atan2
        Min Max Clamp ToLinear ToSrgb Hash IsNan AtMost Select Hint)
}

#[inline(always)]
fn each(a: &Lanes, b: &Lanes, c: &Lanes, out: &mut Lanes, f: impl Fn(f32, f32, f32) -> f32) {
    for i in 0..LANES {
        out[i] = f(a[i], b[i], c[i]);
    }
}
