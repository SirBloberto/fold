use super::Step;

pub fn keep_needed(steps: Vec<Step>, outputs: [u32; 4], slots: u32) -> Vec<Step> {
    let mut needed = vec![false; slots as usize];
    for out in outputs {
        needed[out as usize] = true;
    }
    let mut kept = Vec::new();
    for step in steps.into_iter().rev() {
        if needed[step.out as usize] {
            for arg in step.args {
                needed[arg as usize] = true;
            }
            kept.push(step);
        }
    }
    kept.reverse();
    kept
}

pub fn split(steps: Vec<Step>, slots: u32, pixel_inputs: u32) -> (Vec<Step>, Vec<Step>) {
    let mut varies: Vec<bool> = (0..slots).map(|slot| slot < pixel_inputs).collect();
    let (mut frame, mut pixel) = (Vec::new(), Vec::new());
    for step in steps {
        if step.args.iter().any(|&arg| varies[arg as usize]) {
            varies[step.out as usize] = true;
            pixel.push(step);
        } else {
            frame.push(step);
        }
    }
    (frame, pixel)
}
