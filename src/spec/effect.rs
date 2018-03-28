
#[derive(Debug, Deserialize)]
pub enum EffectSpec {
    #[serde(rename="density")]
    Density {
        width: usize,
        height: usize,
        #[serde(default = "default_bleed")]
        island_bleed: usize,
        tex_pattern: String,
        obj_pattern: Option<String>,
        mtl_pattern: Option<String>
    }
}

fn default_bleed() -> usize {
    2
}
