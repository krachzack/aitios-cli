use runner::SimulationRunner;
use geom::{Vec3, Vertex};
use asset::obj;
use scene::{Entity, Mesh};
use sim::{Simulation, SurfelData, SurfelRule, TonSourceBuilder, TonSource};
use spec::{SimulationSpec, SurfelSpec, SurfelRuleSpec, TonSourceSpec};
use surf::{Surface, SurfaceBuilder, SurfelSampling, Surfel};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;
use std::cmp::Eq;
use std::hash::Hash;
use serde_yaml;

pub fn load<P : Into<PathBuf>>(simulation_spec_file: P) -> Result<SimulationRunner, String> {
    let mut simulation_spec_file = File::open(simulation_spec_file.into())
            .expect("Test simulation spec could not be opened");

    let spec : SimulationSpec = serde_yaml::from_reader(&mut simulation_spec_file)
        .expect("Failed parsing example simulation spec");

    let surfel_specs_by_material_name = surfel_specs_by_material_name(&spec);

    let entities = {
        let scene_path = &spec.scene;
        let entities = obj::load(scene_path);

        match entities {
            Err(_) => return Err(format!("Failed loading scene at: {}", scene_path.file_name().unwrap().to_str().unwrap())),
            Ok(entities) => entities
        }
    };

    let source_specs = load_source_specs(&spec.sources);

    // For faster substance access, each substance name gets an ID which is an
    // index into this vector. Names can occur in sources, and surfels
    // as initial values and as absorption/deposition rates
    let unique_substance_names : HashSet<&String> = surfel_specs_by_material_name.values()
        .flat_map(|s| s.initial.keys().chain(s.deposit.keys()))
        .chain(source_specs.iter().flat_map(|s| s.initial.keys().chain(s.absorb.keys())))
        .collect();
    let unique_substance_names : Vec<String> = unique_substance_names.into_iter().cloned().collect();

    //let surfel_rules = build_surfel_rules(&surfel_specs_by_material_name, &unique_substance_names);
    let sources = build_sources(&source_specs, &unique_substance_names);
    let surface = build_surface(&entities, &surfel_specs_by_material_name, &unique_substance_names, spec.surfel_distance);

    let simulation = {
        let has_fallback_surfel_spec = surfel_specs_by_material_name.contains_key("_");

        // Ignoring geometry where the corresponding material has no surfel specification
        // and hence has no surfels generated, unless a fallback surfel spec is provided
        let all_triangles = entities.iter()
            .filter(|e| has_fallback_surfel_spec || surfel_specs_by_material_name.contains_key(e.material.name()))
            .flat_map(|e| e.mesh.triangles());

        // No global surfel rules, only per substance type mapped from material
        Simulation::new(sources, all_triangles, surface, vec![])
    };

    Ok(SimulationRunner::new(spec, unique_substance_names, simulation, entities))
}

fn load_source_specs(sources: &Vec<PathBuf>) -> Vec<TonSourceSpec> {
    sources.iter()
        .map(|s| {
            let spec_file = &mut File::open(s)
                .expect(&format!("Ton source spec file at \"{:?}\" could not be found", s));

            let spec : TonSourceSpec = serde_yaml::from_reader(spec_file)
                .expect("Parse error for ton source spec");

            spec
        })
        .collect()
}

/*fn build_surfel_rules(surfel_specs_by_material_name: &HashMap<String, SurfelSpec>, unique_substance_names: &Vec<String>) -> Vec<SurfelRule> {
    surfel_specs_by_material_name.values()
        .flat_map(|s| s.rules.iter())
        .map(|s| match s {
            &SurfelRuleSpec::Transfer { ref from, ref to, factor } =>
                SurfelRule::Transfer {
                    source_substance_idx: unique_substance_names.iter().position(|n| n == from)
                        .expect(&format!("Surfel transport rule references unknown substance name {}", from)),
                    target_substance_idx: unique_substance_names.iter().position(|n| n == to)
                        .expect(&format!("Surfel transport rule references unknown substance name {}", to)),
                    factor
                },
            &SurfelRuleSpec::Deteriorate { ref from, factor } =>
                SurfelRule::Deteriorate {
                    substance_idx: unique_substance_names.iter().position(|n| n == from)
                        .expect(&format!("Surfel transport rule references unknown substance name {}", from)),
                    factor
                }
        })
        .collect()
}*/

fn build_sources(sources: &Vec<TonSourceSpec>, unique_substance_names: &Vec<String>) -> Vec<TonSource> {
    sources.iter()
        .map(|spec| {
            let mesh = &obj::load(&spec.mesh)
                .expect(&format!("Ton source mesh could not be loaded at \"{:?}\"", spec.mesh))[0].mesh;

            let mut builder = TonSourceBuilder::new();

            if let Some(ref direction_arr) = spec.flow_direction {
                builder = builder.flow_direction_static(Vec3::new(direction_arr[0], direction_arr[1], direction_arr[2]));
            }

            builder.mesh_shaped(mesh, spec.diffuse)
                .emission_count(spec.emission_count)
                .p_straight(spec.p_straight)
                .p_parabolic(spec.p_parabolic)
                .p_flow(spec.p_flow)
                .substances(&extract_keys(&spec.initial, unique_substance_names, 0.0))
                .pickup_rates(extract_keys(&spec.absorb, unique_substance_names, 0.0))
                .interaction_radius(spec.interaction_radius)
                .parabola_height(spec.parabola_height)
                .flow_distance(spec.flow_distance)
                .build()
        })
        .collect()
}

fn surfel_specs_by_material_name(spec: &SimulationSpec) -> HashMap<String, SurfelSpec> {
    spec.surfels_by_material.iter()
        .map(|(key, val)| {
            // FIXME error handling
            let surfel_file = &mut File::open(val)
                .expect(&format!("Surfel spec could not be found at {:?}", val));

            let surfel_spec : SurfelSpec = serde_yaml::from_reader(surfel_file)
                .unwrap();

            (key.clone(), surfel_spec)
        })
        .collect()
}

fn build_surface(entities: &Vec<Entity>, surfel_specs_by_material_name: &HashMap<String, SurfelSpec>, unique_substance_names: &Vec<String>, surfel_distance: f32) -> Surface<Surfel<Vertex, SurfelData>> {
    let catchall_surfel_spec = surfel_specs_by_material_name.get("_");
    let default_substance_concentration = 0.0;
    let default_deposition_rate = 0.0;

    entities.iter()
        .enumerate()
        .fold(
            SurfaceBuilder::new()
                // TODO make this configurable
                .sampling(SurfelSampling::MinimumDistance(surfel_distance)),
            |b, (entity_idx, ent)| {
                let material_name = ent.material.name();

                let surfel_spec = surfel_specs_by_material_name.get(material_name)
                    .or(catchall_surfel_spec);

                if let Some(surfel_spec) = surfel_spec {
                    let rules = surfel_spec.rules.iter()
                        .map(|s| match s {
                            &SurfelRuleSpec::Transfer { ref from, ref to, factor } =>
                                SurfelRule::Transfer {
                                    source_substance_idx: unique_substance_names.iter().position(|n| n == from)
                                        .expect(&format!("Surfel transport rule references unknown substance name {}", from)),
                                    target_substance_idx: unique_substance_names.iter().position(|n| n == to)
                                        .expect(&format!("Surfel transport rule references unknown substance name {}", to)),
                                    factor
                                },
                            &SurfelRuleSpec::Deteriorate { ref from, factor } =>
                                SurfelRule::Deteriorate {
                                    substance_idx: unique_substance_names.iter().position(|n| n == from)
                                        .expect(&format!("Surfel transport rule references unknown substance name {}", from)),
                                    factor
                                }
                        })
                        .collect();

                    let proto_surfel = SurfelData {
                        entity_idx,
                        delta_straight: surfel_spec.reflectance.delta_straight,
                        delta_parabolic: surfel_spec.reflectance.delta_parabolic,
                        delta_flow: surfel_spec.reflectance.delta_flow,
                        substances: extract_keys(&surfel_spec.initial, &unique_substance_names, default_substance_concentration),
                        /// Weights for the transport of substances from a settled ton to a surfel
                        deposition_rates: extract_keys(&surfel_spec.deposit, &unique_substance_names, default_deposition_rate),
                        rules
                    };

                    info!("Sampling entity \"{}\" into surfel representation, 2r={}…", ent.name, surfel_distance);

                    b.sample_triangles(
                        ent.mesh.triangles(),
                        &proto_surfel
                    )
                } else {
                    // If no surfel spec is defined in the YAML, ignore the entity for the simulation
                    b
                }
            }
        )
        .build()
}

/// Extracts the values of the given vector of keys from the given map.
/// If no value is found under the given key, the given default is stored in its place.
fn extract_keys<K : Eq + Hash, V : Clone>(map: &HashMap<K, V>, keys: &Vec<K>, default: V) -> Vec<V> {
    keys.iter()
        .map(|k| map.get(k).unwrap_or(&default).clone())
        .collect()
}
