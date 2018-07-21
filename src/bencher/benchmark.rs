use super::msg::Msg;
use std::sync::mpsc::Sender;
use std::time::SystemTime;

pub struct Benchmark {
    start_time: SystemTime,
    tx: Sender<Msg>,
}

impl Benchmark {
    pub fn new(tx: Sender<Msg>) -> Self {
        Self {
            start_time: SystemTime::now(),
            tx,
        }
    }
}

impl Drop for Benchmark {
    fn drop(&mut self) {
        match self.start_time.elapsed() {
            Ok(elapsed) => self
                .tx
                .send(Msg::Persist(elapsed))
                .expect("Could not send benchmarked time to worker"),
            Err(err) => error!("Benchmarking failed {}", err),
        }
    }
}
