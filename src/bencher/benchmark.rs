use super::msg::Msg;
use bencher::Bencher;
use std::marker::PhantomData;
use std::sync::mpsc::Sender;
use std::time::SystemTime;

/// A benchmark running in a bencher.
/// There is not really a reference to the bencher,
/// but the lifetime should make sure the benchmark
/// is dropped before the bencher.
pub struct Benchmark<'a> {
    bencher: PhantomData<&'a Bencher>,
    start_time: SystemTime,
    tx: Sender<Msg>,
}

impl<'a> Benchmark<'a> {
    pub fn new(tx: Sender<Msg>) -> Self {
        Self {
            bencher: PhantomData,
            start_time: SystemTime::now(),
            tx,
        }
    }
}

impl<'a> Drop for Benchmark<'a> {
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
