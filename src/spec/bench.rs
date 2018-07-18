use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct BenchSpec {
    pub iterations: Option<PathBuf>
}
