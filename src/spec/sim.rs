use std::collections::HashMap;
use std::path::PathBuf;
use spec::EffectSpec;

#[derive(Debug, Deserialize)]
pub struct SimulationSpec {
    pub name: String,
    pub description: String,
    pub scene: PathBuf,
    pub iterations: usize,
    pub log: Option<PathBuf>,
    #[serde(default = "default_surfel_distance")]
    pub surfel_distance: f32,
    pub sources: Vec<PathBuf>,
    pub surfels_by_material: HashMap<String, String>,
    pub effects: Vec<EffectSpec>
}

fn default_surfel_distance() -> f32 {
    0.01
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_yaml;
    use std::fs::File;

    #[test]
    fn test_parse_simulation_spec() {
        let path = "tests/examples/simulation.yml";
        let mut simulation_spec_file = File::open(path)
            .expect("Test simulation spec could not be opened");

        let spec : SimulationSpec = serde_yaml::from_reader(&mut simulation_spec_file)
            .expect("Failed parsing example simulation spec");

        assert_eq!(spec.name, "Park Scene");
        assert_eq!(spec.scene.file_name().unwrap().to_str().unwrap(), "buddha.obj");
        assert_eq!(spec.iterations, 30);
        assert_eq!(spec.surfels_by_material.get("bronze").unwrap(), "iron.yml");
        assert_eq!(spec.surfels_by_material.get("_").unwrap(), "concrete.yml");
        assert_eq!(spec.sources[0].file_name().unwrap().to_str().unwrap(), "rain.yml");

        match &spec.effects[0] {
            &EffectSpec::Density {
                ref tex_pattern,
                ref obj_pattern,
                ref mtl_pattern,
                ..
            } => {
                assert_eq!(tex_pattern, "test-output/test-{datetime}/iteration-{iteration}/{id}-{entity}-{substance}.png");
                assert_eq!(obj_pattern.as_ref().unwrap(), "test-output/test-{datetime}/iteration-{iteration}/{substance}.obj");
                assert_eq!(mtl_pattern.as_ref().unwrap(), "test-output/test-{datetime}/iteration-{iteration}/{substance}.mtl");
            },
            _ => ()
        }

    }
}
