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
