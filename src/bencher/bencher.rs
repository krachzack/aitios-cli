use std::io::Write;
use std::thread::{spawn, JoinHandle};
use std::sync::mpsc::{channel, Receiver, Sender};
use super::Benchmark;
use super::msg::Msg;

pub struct Bencher {
    tx: Sender<Msg>,
    worker_handle: Option<JoinHandle<()>>
}

impl Bencher {
    /// Spawns a new bencher with a worker thread writing benchmarks to the
    /// specified sink.
    pub fn new<W>(sink: W) -> Self
        where W : Write + Send + 'static
    {
        let (tx, rx) = channel();
        let worker_handle = Some(spawn(move || persist_benchmarks(rx, sink)));
        Self {
            tx, worker_handle
        }
    }

    /// Measures a benchmark.
    ///
    /// It runs until the returned handle is dropped and is then
    /// asynchroneously persisted by the worker thread.
    ///
    /// # Panics
    /// No benchmarks are possible after the bancher has been flushed.
    /// Panics if called after `bencher.flush()`.
    pub fn bench(&self) -> Benchmark {
        if self.worker_handle.is_none() {
            panic!("Tried to benchmark but Bencher has already been flushed.")
        }

        Benchmark::new(self.tx.clone())
    }

    /// Finishes the benchmark and makes sure everything has been
    /// persisted.
    ///
    /// No new benchmarks can be persisted after this point.
    pub fn flush(&mut self) {
        // Only flush if not already flushed
        if let Some(handle) = self.worker_handle.take() {
            // Tell the worker to shut down
            self.tx.send(Msg::Done)
                .expect("Could not send benchmark worker thread Done message.");

            // Wait for the worker shutdown so the file is guaranteed
            // to exist.
            handle.join()
                .expect("Bencher could not wait for worker thread to finish.")
        }
    }
}

impl Drop for Bencher {
    /// Make sure worker is finished when dropping bencher.
    fn drop(&mut self) {
        // Tell the worker to orderly shut down
        self.flush();
    }
}

fn persist_benchmarks<W>(rx: Receiver<Msg>, mut sink: W)
    where W : Write
{
    while let Ok(Msg::Persist(duration)) = rx.recv() {
        let secs = duration.as_secs();
        let nanos = duration.subsec_nanos();

        // Pad nanos with zeros to nine digits to make
        // a number in seconds out of it.
        writeln!(
            sink,
            "{}.{:09}", secs, nanos
        ).expect("Could not write to benchmark sink.");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Read;
    use std::path::Path;
    use std::fs::{File, remove_file};
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn persistence() {
        let csv_path = &Path::new("/tmp/benchmark_persistence_test.csv");

        {
            let csv = File::create(csv_path)
                .expect("Could not create test CSV for benchmarking.");
            let bencher = Bencher::new(csv);

            {
                let _benchmark_10 = bencher.bench();
                sleep(Duration::from_millis(10));
            }

            {
                let _benchmark_0 = bencher.bench();
            }

            {
                let _benchmark_15 = bencher.bench();
                sleep(Duration::from_millis(1015));
            }
        }

        let mut benchmark_output = String::new();
        let mut benchmark_file = File::open(csv_path)
            .expect("Did not find a file created by the benchmarker.");
        benchmark_file.read_to_string(&mut benchmark_output)
            .expect("Could not read file created by benchmarker to string");

        assert_eq!(
            3,
            benchmark_output.lines().count(),
            "Expected three benchmarks."
        );


        let mut benchmarks_ms = benchmark_output.lines()
            // Extract second column (subsecond part in nanoseconds)
            .map(|l| l.split('.')
                .map(str::parse::<u64>)
                .collect::<Result<Vec<u64>, _>>()
                .expect("Benchmarker output could not be parsed as semicolon-separated unsigned numbers")
            )
            // Make tuple of (seconds, subsecond-milliseconds) out of each line
            .map(|l| (l[0], l[1] / 1000000));

        assert_eq!(Some((0, 10)), benchmarks_ms.next(), "Expected first benchmark to measure 10ms.");
        assert_eq!(Some((0, 0)),  benchmarks_ms.next(), "Expected second benchmark to measure 0ms.");
        assert_eq!(Some((1, 15)), benchmarks_ms.next(), "Expected third benchmark to measure 1015ms.");

        remove_file(csv_path)
            .expect("Could not remove test CSV file.");
    }
}
