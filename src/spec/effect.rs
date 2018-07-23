use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub enum EffectSpec {
    #[serde(rename = "density")]
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
        mtl_pattern: Option<String>,
    },
    /// Writes the scene with the effects before the declaration to the
    /// given paths. This should usually the last step, but exporting
    /// before can be useful for debugging.
    #[serde(rename = "export")]
    Export {
        obj_pattern: Option<String>,
        mtl_pattern: Option<String>,
    },
    /// Uses the concentration of the substance with the given name to create
    /// new textures for all entities with a material that has a name equal to
    /// one of the ones specified in the materials list.
    ///
    /// Allowed map types are normal, displacement, albedo, metallicity and
    /// roughness. Each use a specialiced blending technique and are specified
    /// with the path to a sample texture and an influence factor.
    ///
    /// Width and height of the output textures will be:
    /// 1. The defined width, height, or both beside the blend stops (highest precedence),
    /// 2. if unspecified, dimensions of the original map in the base material,
    /// 3. if no original map defined in entity material, use the dimensions of the largest sample texture.
    ///
    /// Multiple layer effects are allowed and will be applied in declaration order.
    #[serde(rename = "layer")]
    Layer {
        /// A list of material names where on each entity that uses it, a new material will be derived to replace it.
        materials: Vec<String>,
        /// The name of the substance that defines the texel concentration.
        substance: String,
        #[serde(default = "default_surfel_lookup")]
        surfel_lookup: SurfelLookup,
        #[serde(default = "default_bleed")]
        island_bleed: usize,
        // REVIEW should normal and displacement be usable together? maybe the normal map should be derived from the displacement map to ensure consistency
        normal: Option<Blend>,
        displacement: Option<Blend>,
        albedo: Option<Blend>,
        metallicity: Option<Blend>,
        roughness: Option<Blend>,
    },
    #[serde(rename = "dump_surfels")]
    DumpSurfels { obj_pattern: String },
}

#[derive(Debug, Deserialize, Clone)]
pub struct Blend {
    /// If specified, use this output texture width instead
    /// of the width of the original map from the material or
    /// the largest blendstop.
    ///
    /// If used without height, uses the same value for height.
    pub width: Option<usize>,
    /// If specified, use this output texture height instead
    /// of the height of the original map from the material or
    /// the largest blendstop.
    ///
    /// If used without width, uses the same value for width.
    pub height: Option<usize>,
    /// Texture samples at specified concentrations. Unless explicitly specified, cenith
    /// 0.0 is populated automatically with the original texture if left unspecified.
    /// If no stop is specified, the implicit stop at 0.0 will trigger the use of the
    /// original texture without blending. This can still be useful with an influence
    /// lower than one to blend over the original texture after preceding blending.
    pub stops: Vec<Stop>,
    /// Multiplier for the blending the newly created texture together with the original texture.
    /// Influence 0 leaves the original texture completely intact, the default of 1 replaces the
    /// original texture completely with the blended version.
    /// Note that texture samples may also be partly transparent.
    #[serde(default = "default_influence")]
    pub influence: f32,
    /// {entity} {iteration} {id} {substance}
    pub tex_pattern: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Stop {
    /// Path to the texture sample.
    pub sample: Option<PathBuf>,
    /// The concentration where this texture has maximum influence.
    /// To interpolate a given concentration, interpolation is performed
    /// between the textures at the cenith before and after.
    pub cenith: f32,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(untagged)]
pub enum SurfelLookup {
    Nearest { count: usize },
    Within { within: f32 },
}

fn default_bleed() -> usize {
    2
}

fn default_influence() -> f32 {
    1.0
}

fn default_surfel_lookup() -> SurfelLookup {
    SurfelLookup::Nearest { count: 6 }
}
