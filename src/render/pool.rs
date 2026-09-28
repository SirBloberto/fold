use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

type Job = Arc<dyn Fn() + Send + Sync>;

pub struct Pool {
    senders: Vec<Sender<Job>>,
    done: Receiver<()>,
    workers: Vec<JoinHandle<()>>,
}

impl Pool {
    pub fn new(threads: usize) -> Pool {
        let (finished, done) = channel();
        let mut senders = Vec::new();
        let mut workers = Vec::new();
        for _ in 1..threads {
            let (sender, jobs) = channel::<Job>();
            let finished = finished.clone();
            workers.push(std::thread::spawn(move || {
                for job in jobs {
                    job();
                    drop(job);
                    finished.send(()).expect("the pool outlives its jobs");
                }
            }));
            senders.push(sender);
        }
        Pool {
            senders,
            done,
            workers,
        }
    }

    pub fn run(&self, job: impl Fn() + Send + Sync + 'static) {
        let job: Job = Arc::new(job);
        for sender in &self.senders {
            sender.send(job.clone()).expect("workers wait for jobs");
        }
        job();
        for _ in &self.senders {
            self.done.recv().expect("every worker finishes its job");
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.senders.clear();
        for worker in self.workers.drain(..) {
            worker.join().expect("a worker stops cleanly");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn every_thread_runs_each_job() {
        let pool = Pool::new(4);
        let count = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let count = count.clone();
            pool.run(move || {
                count.fetch_add(1, Ordering::Relaxed);
            });
        }
        assert_eq!(count.load(Ordering::Relaxed), 12);
    }
}
