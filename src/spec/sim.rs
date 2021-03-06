use spec::{BenchSpec, EffectSpec, SurfelRuleSpec, Transport};
use std::collections::HashMap;
use std::default::Default;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct SimulationSpec {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scenes: Vec<PathBuf>,
    pub iterations: Option<u32>,
    /// Determines how often the effect pipeline is run.
    /// Iteration 0 and the last iteration will always be run,
    /// regardless of this setting.
    pub effect_interval: Option<u32>,
    pub log: Option<PathBuf>,
    pub surfel_distance: Option<f32>,
    #[serde(default)]
    pub sources: Vec<PathBuf>,
    #[serde(default)]
    pub surfels_by_material: HashMap<String, String>,
    #[serde(default)]
    pub effects: Vec<EffectSpec>,
    pub benchmark: Option<BenchSpec>,
    pub transport: Option<Transport>,
    pub flat_filtering: Option<bool>,
    #[serde(default)]
    pub rules: Vec<SurfelRuleSpec>,
}

impl Default for SimulationSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            scenes: Vec::new(),
            iterations: None,
            effect_interval: None,
            log: None,
            surfel_distance: None,
            sources: Vec::new(),
            surfels_by_material: HashMap::new(),
            effects: Vec::new(),
            benchmark: None,
            transport: None,
            flat_filtering: None,
            rules: Vec::new(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_yaml;
    use std::fs::File;

    #[test]
    fn test_parse_simulation_spec() {
        let path = "tests/examples/simulation.yml";
        let mut simulation_spec_file =
            File::open(path).expect("Test simulation spec could not be opened");

        let spec: SimulationSpec = serde_yaml::from_reader(&mut simulation_spec_file)
            .expect("Failed parsing example simulation spec");

        assert_eq!(spec.name, "Park Scene");
        assert!(
            spec.scenes
                .iter()
                .all(|scene| scene.file_name().unwrap().to_str().unwrap() == "buddha.obj"),
        );
        assert_eq!(spec.iterations, Some(30));
        assert_eq!(spec.surfels_by_material.get("bronze").unwrap(), "iron.yml");
        assert_eq!(spec.surfels_by_material.get("_").unwrap(), "concrete.yml");
        assert_eq!(
            spec.sources[0].file_name().unwrap().to_str().unwrap(),
            "rain.yml"
        );

        match &spec.effects[0] {
            &EffectSpec::Density {
                ref tex_pattern,
                ref obj_pattern,
                ref mtl_pattern,
                ..
            } => {
                assert_eq!(tex_pattern, "test-output/test-{datetime}/iteration-{iteration}/{id}-{entity}-{substance}.png");
                assert_eq!(
                    obj_pattern.as_ref().unwrap(),
                    "test-output/test-{datetime}/iteration-{iteration}/{substance}.obj"
                );
                assert_eq!(
                    mtl_pattern.as_ref().unwrap(),
                    "test-output/test-{datetime}/iteration-{iteration}/{substance}.mtl"
                );
            }
            _ => (),
        }
    }
}
