use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct SurfelSpec {
    pub name: String,
    description: String,
    pub reflectance: TonReflectance,
    pub initial: HashMap<String, f32>,
    pub deposit: HashMap<String, f32>,
    // TODO only global surfel rules allowed as of yet
    #[serde(default = "Vec::new")]
    pub rules: Vec<SurfelRuleSpec>,
}

#[derive(Debug, Deserialize)]
pub struct TonReflectance {
    pub delta_straight: f32,
    pub delta_parabolic: f32,
    pub delta_flow: f32,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum SurfelRuleSpec {
    Transfer {
        from: String,
        to: String,
        factor: f32,
    },
    Deteriorate {
        from: String,
        factor: f32,
    },
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_yaml;
    use std::fs::File;

    #[test]
    fn test_parse_surfel_spec() {
        let path = "tests/examples/iron.yml";
        let mut surfel_spec_file = File::open(path).expect("Test surfel spec could not be opened");

        let spec: SurfelSpec = serde_yaml::from_reader(&mut surfel_spec_file)
            .expect("Failed parsing example surfel spec");

        assert_eq!(spec.name, "Iron");
        assert_eq!(spec.reflectance.delta_straight, 0.0);
        assert_eq!(spec.reflectance.delta_parabolic, 0.8);
        assert_eq!(spec.reflectance.delta_flow, 0.2);
        assert_eq!(*spec.initial.get("humidity").unwrap(), 0.0);
        assert_eq!(*spec.initial.get("rust").unwrap(), 0.0);
        assert_eq!(*spec.deposit.get("humidity").unwrap(), 1.0);
        assert_eq!(*spec.deposit.get("rust").unwrap(), 0.5);

        match &spec.rules[1] {
            &SurfelRuleSpec::Deteriorate { ref from, factor } => {
                assert_eq!(from, "humidity");
                assert_eq!(factor, -0.5);
            }
            _ => assert!(false, "Did expect unary rule second"),
        }

        match &spec.rules[0] {
            &SurfelRuleSpec::Transfer {
                ref from,
                ref to,
                factor,
            } => {
                assert_eq!(from, "humidity");
                assert_eq!(to, "rust");
                assert_eq!(factor, 0.5);
            }
            _ => assert!(false, "Did expect binary rule first"),
        }
    }
}
