
#[derive(Debug, Deserialize)]
pub enum EffectSpec {
    #[serde(rename="density")]
    Density {
        width: usize,
        height: usize,
        /// Technique for looking up the surfels that correspond to a texel.
        /// Either a fixed count or within some world space radius
        #[serde(default = "default_surfel_lookup")]
        surfel_lookup: SurfelLookup,
        #[serde(default = "default_bleed")]
        island_bleed: usize,
        tex_pattern: String,
        obj_pattern: Option<String>,
        mtl_pattern: Option<String>
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(untagged)]
pub enum SurfelLookup {
    Nearest { count: usize },
    Within { within: f32 }
}

fn default_bleed() -> usize {
    2
}

fn default_surfel_lookup() -> SurfelLookup {
    SurfelLookup::Nearest { count: 6 }
}
