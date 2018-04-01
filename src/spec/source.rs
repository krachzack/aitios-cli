use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct TonSourceSpec {
    name: String,
    description: String,
    pub mesh: PathBuf,
    pub emission_count: usize,
    #[serde(default = "is_diffuse_default")]
    pub diffuse: bool,
    pub p_straight: f32,
    pub p_parabolic: f32,
    pub p_flow: f32,
    /// Initial concentrations by material name
    pub initial: HashMap<String, f32>,
    /// When bouncing, not settling, indicates how much mateiral is absorbed from surfels
    pub absorb: HashMap<String, f32>,
    pub interaction_radius: f32,
    pub parabola_height: f32,
    pub flow_distance: f32,
    /// If set, provides direction of flow that is projected onto triangles to obtain
    /// final flow direction. If left out, incoming direction will be projected.
    pub flow_direction: Option<[f32; 3]>
}

fn is_diffuse_default() -> bool { false }

#[cfg(test)]
mod test {
    use super::*;
    use serde_yaml;
    use std::fs::File;

    #[test]
    fn test_parse_ton_source_spec() {
        let path = "tests/examples/rain.yml";
        let mut ton_source_spec_file = File::open(path)
            .expect("Test ton source spec could not be opened");

        let spec : TonSourceSpec = serde_yaml::from_reader(&mut ton_source_spec_file)
            .expect("Failed parsing example ton source spec");

        assert_eq!(spec.name, "Rain");
        assert_eq!(spec.description, "Rain dropping from the sky");
        assert_eq!(spec.mesh.file_name().unwrap().to_str().unwrap(), "sky.obj");
        assert_eq!(spec.emission_count, 100000);
        assert_eq!(spec.p_straight, 0.0);
        assert_eq!(spec.p_parabolic, 0.3);
        assert_eq!(spec.p_flow, 0.7);
        assert_eq!(*spec.initial.get("humidity").unwrap(), 1.0);
        assert_eq!(*spec.initial.get("rust").unwrap(), 0.0);
        assert_eq!(*spec.absorb.get("humidity").unwrap(), 1.0);
        assert_eq!(*spec.absorb.get("rust").unwrap(), 0.2);
        assert_eq!(spec.interaction_radius, 0.1);
        assert_eq!(spec.parabola_height, 0.07);
        assert_eq!(spec.flow_distance, 0.17);
        assert_eq!(spec.flow_direction, Some([0.0, -1.0, 0.0]));
    }
}
