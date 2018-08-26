#[derive(Debug, Copy, Clone, Deserialize)]
pub enum Transport {
    #[serde(rename = "classic")]
    Classic,
    #[serde(rename = "consistent")]
    Consistent,
    #[serde(rename = "conserving")]
    Conserving,
    #[serde(rename = "differential")]
    Differential,
}
