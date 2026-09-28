use super::batch::{Batch, Lanes};
use super::{Step, TIME, Tape, X, Y, execute};

const ON_X: u8 = 1;
const ON_Y: u8 = 2;
const ON_TIME: u8 = 4;

#[derive(Debug)]
pub struct Kernel {
    pub slots: u32,
    pub constants: Vec<(u32, f32)>,
    pub frame: Vec<Step>,
    pub row: Vec<Step>,
    pub batch: Batch,
    pub changes: bool,
}

#[derive(Default)]
pub struct Scratch {
    values: Vec<f32>,
    lanes: Vec<Lanes>,
}

impl Kernel {
    pub fn new(tape: &Tape) -> Kernel {
        let mut on = vec![0; tape.slots as usize];
        on[X] = ON_X;
        on[Y] = ON_Y;
        on[TIME] = ON_TIME;
        let (mut frame, mut row, mut column, mut pixel) = (vec![], vec![], vec![], vec![]);
        for &step in &tape.steps {
            let depends = step.args.iter().fold(0, |d, &arg| d | on[arg as usize]);
            on[step.out as usize] = depends;
            let level = match (depends & ON_X != 0, depends & ON_Y != 0) {
                (false, false) => &mut frame,
                (false, true) => &mut row,
                (true, false) => &mut column,
                (true, true) => &mut pixel,
            };
            level.push(step);
        }
        let by_row: Vec<bool> = on.iter().map(|&d| d & ON_Y != 0).collect();
        Kernel {
            slots: tape.slots,
            constants: tape.constants.clone(),
            frame,
            row,
            batch: Batch::new(&column, &pixel, tape.outputs, &by_row),
            changes: tape
                .outputs
                .iter()
                .any(|&out| on[out as usize] & ON_TIME != 0),
        }
    }

    pub fn run(
        &self,
        scratch: &mut Scratch,
        time: f32,
        x: &Lanes,
        ys: impl IntoIterator<Item = (usize, f32)>,
        mut each: impl FnMut(usize, [&Lanes; 4]),
    ) {
        let Scratch { values, lanes } = scratch;
        values.resize(self.slots as usize, 0.0);
        for &(slot, n) in &self.constants {
            values[slot as usize] = n;
        }
        values[TIME] = time;
        execute(&self.frame, values);
        self.batch.start(lanes, values, x);
        for (row, y) in ys {
            values[Y] = y;
            execute(&self.row, values);
            each(row, self.batch.run(lanes, values));
        }
    }
}
