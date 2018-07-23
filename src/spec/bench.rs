use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct BenchSpec {
    pub iterations: Option<PathBuf>,
    pub tracing: Option<PathBuf>,
    pub synthesis: Option<PathBuf>,
    pub setup: Option<PathBuf>,
}
