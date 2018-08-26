use asset::obj;
use builder::{Error, ResolveErrorKind};
use chrono::*;
use files::{create_file_recursively, fs_timestamp, Resolver};
use geom::{TupleTriangle, Vec3, Vertex};
use runner::SimulationRunner;
use scene::DeinterleavedIndexedMeshBuf;
use scene::{Entity, Mesh};
use serde_yaml;
use sim::{Config, Simulation, SurfelData, SurfelRule, TonSource, TonSourceBuilder, Transport};
use spec::{BenchSpec, SimulationSpec, SurfelRuleSpec, SurfelSpec, TonSourceSpec, Transport::*};
use std::cmp::Eq;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::Hash;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;
use surf::{Surface, SurfaceBuilder, Surfel, SurfelSampling};

/// Makes a simulation runner according to the given spec.
///
/// The runner contains the spec. The spec will be mutated in some places,
/// e.g. contained paths will be canonicalized.
///
/// TODO this resolving business needs to be removed, since canonicalize
///      is now responsible for this.
pub fn instantiate(
    spec: SimulationSpec,
    resolver: &Resolver,
    creation_time: DateTime<Local>,
) -> Result<SimulationRunner, Error> {
    let load_start_time = SystemTime::now();

    let surfel_specs_by_material_name = surfel_specs_by_material_name(&spec, &resolver)?;

    let entities = load_entities(&spec.scenes, &surfel_specs_by_material_name)?;

    let source_specs = load_source_specs(&spec.sources, &resolver)?;

    let unique_substance_names =
        unique_substance_names(&surfel_specs_by_material_name, &source_specs);

    if unique_substance_names.is_empty() {
        return Err(Error::SubstancesMissing);
    }

    if spec.effects.is_empty() {
        return Err(Error::EffectsMissing);
    }

    //let surfel_rules = build_surfel_rules(&surfel_specs_by_material_name, &unique_substance_names);
    let sources = build_sources(&source_specs, &unique_substance_names, &resolver)?;

    let surfel_distance = spec.surfel_distance;
    if surfel_distance.is_none() || surfel_distance.unwrap() <= 0.0 {
        return Err(Error::InvalidSurfelDistance(surfel_distance));
    }
    let surface = build_surface(
        &entities,
        &surfel_specs_by_material_name,
        &unique_substance_names,
        surfel_distance.unwrap(),
    );

    let simulation = {
        let has_fallback_surfel_spec = surfel_specs_by_material_name.contains_key("_");

        // Ignoring geometry where the corresponding material has no surfel specification
        // and hence has no surfels generated, unless a fallback surfel spec is provided
        let all_triangles = entities
            .iter()
            .filter(|e| {
                has_fallback_surfel_spec
                    || surfel_specs_by_material_name.contains_key(e.material.name())
            })
            .flat_map(|e| e.mesh.triangles());

        let transport = match spec.transport {
            Some(Classic) => Transport::classic(),
            Some(Consistent) => Transport::consistent(),
            Some(Conserving) => Transport::conserving(),
            Some(Differential) | None => Transport::differential(),
        };

        let config = Config { transport };

        let rules = spec
            .rules
            .iter()
            .map(|r| rule_by_spec(r, &unique_substance_names))
            .collect();

        // No global surfel rules, only per substance type mapped from material
        Simulation::new_with_config(config, sources, all_triangles, surface, rules)
    };

    let datetime = fs_timestamp(creation_time);
    let runner = SimulationRunner::new(
        spec,
        unique_substance_names,
        simulation,
        entities,
        &datetime,
    );

    if let Some(BenchSpec {
        setup: Some(ref setup_csv),
        ..
    }) = runner.spec().benchmark
    {
        let elapsed = load_start_time.elapsed().unwrap();
        let secs = elapsed.as_secs();
        let nanos = elapsed.subsec_nanos();

        let mut setup_csv = create_file_recursively(
            setup_csv.to_str().unwrap().replace("{datetime}", &datetime),
        ).expect("Could not write to benchmark sink.");

        writeln!(setup_csv, "{}.{:09}", secs, nanos).expect("Could not write to benchmark sink.");
    }

    Ok(runner)
}

fn load_entities(
    paths: &Vec<PathBuf>,
    surfel_specs_by_material_name: &HashMap<String, SurfelSpec>,
) -> Result<Vec<Entity>, Error> {
    let mut all_entities = Vec::new();

    for scene_path in paths.iter() {
        let mut entities = obj::load(&scene_path)?;

        // Throw out all entitites which have no mapped surfel spec,
        // unless there is a fallback material named "_".
        // This ignoring affects intersection test and surfel generation,
        // potentially providing a massive speedup if many objects ignored.
        if !surfel_specs_by_material_name.contains_key("_") {
            entities.retain(|e| {
                surfel_specs_by_material_name
                    .keys()
                    .any(|n| n == e.material.name())
            });
        }

        all_entities.extend(entities);
    }

    Ok(all_entities)
}

/// For faster substance access, each substance name gets an ID which is an
/// index into the returned vector. Names can occur in sources, and surfels
/// as initial values and as absorption/deposition rates
fn unique_substance_names(
    surfel_specs: &HashMap<String, SurfelSpec>,
    source_specs: &Vec<TonSourceSpec>,
) -> Vec<String> {
    let unique_substance_names: HashSet<&String> = surfel_specs
        .values()
        .flat_map(|s| s.initial.keys().chain(s.deposit.keys()))
        .chain(
            source_specs
                .iter()
                .flat_map(|s| s.initial.keys().chain(s.absorb.keys())),
        )
        .collect();

    unique_substance_names.into_iter().cloned().collect()
}

fn load_source_specs(
    sources: &Vec<PathBuf>,
    resolver: &Resolver,
) -> Result<Vec<TonSourceSpec>, Error> {
    /*if sources.is_empty() {
        return Err(Error::SourcesMissing);
    }*/

    sources
        .iter()
        .map(|s| load_source_spec(s, resolver))
        .collect()
}

fn load_source_spec(path: &PathBuf, resolver: &Resolver) -> Result<TonSourceSpec, Error> {
    let path = resolver
        .resolve(path)
        .map_err(|e| Error::resolve(e, ResolveErrorKind::TonSourceSpec))?;

    let spec_file = &mut File::open(path)?;

    let spec: TonSourceSpec = serde_yaml::from_reader(spec_file)?;

    Ok(spec)
}

fn build_sources(
    sources: &Vec<TonSourceSpec>,
    unique_substance_names: &Vec<String>,
    resolver: &Resolver,
) -> Result<Vec<TonSource>, Error> {
    sources
        .iter()
        .map(|spec| {
            let mesh_scene = resolver
                .resolve(&spec.mesh)
                .map_err(|e| Error::resolve(e, ResolveErrorKind::TonSourceMesh))?;

            let mesh_scene = &obj::load(&mesh_scene)?;

            let mesh = if mesh_scene.len() == 0 {
                panic!("Emission mesh scene does not contain any entities")
            } else if mesh_scene.len() == 1 {
                Rc::clone(&mesh_scene.into_iter().next().unwrap().mesh)
            } else {
                // Combine everything in the source mesh scene into a megamesh
                // when encountering more than one entity
                Rc::new(
                    mesh_scene
                        .iter()
                        .flat_map(|m| {
                            m.mesh.triangles().flat_map(|t| {
                                let TupleTriangle(v0, v1, v2) = t;
                                vec![v0, v1, v2].into_iter()
                            })
                        })
                        .collect::<DeinterleavedIndexedMeshBuf>(),
                )
            };

            let mut builder = TonSourceBuilder::new();

            if let Some(ref direction_arr) = spec.flow_direction {
                builder = builder.flow_direction_static(Vec3::new(
                    direction_arr[0],
                    direction_arr[1],
                    direction_arr[2],
                ));
            }

            let source = builder
                .mesh_shaped(&mesh, spec.diffuse)
                .emission_count(spec.emission_count)
                .p_straight(spec.p_straight)
                .p_parabolic(spec.p_parabolic)
                .p_flow(spec.p_flow)
                .substances(&extract_keys(&spec.initial, unique_substance_names, 0.0))
                .pickup_rates(extract_keys(&spec.absorb, unique_substance_names, 0.0))
                .interaction_radius(spec.interaction_radius)
                .parabola_height(spec.parabola_height)
                .flow_distance(spec.flow_distance)
                .build();

            Ok(source)
        })
        .collect()
}

fn surfel_specs_by_material_name(
    spec: &SimulationSpec,
    resolver: &Resolver,
) -> Result<HashMap<String, SurfelSpec>, Error> {
    let mut specs = HashMap::with_capacity(spec.surfels_by_material.len());

    for (material_name, surfel_spec) in spec.surfels_by_material.iter() {
        let surfel_spec = &mut File::open(
            resolver
                .resolve(surfel_spec)
                .map_err(|e| Error::resolve(e, ResolveErrorKind::SurfelSpec))?,
        )?;

        let surfel_spec: SurfelSpec = serde_yaml::from_reader(surfel_spec)?;

        specs.insert(material_name.clone(), surfel_spec);
    }

    if specs.is_empty() {
        Err(Error::SurfelSpecsMissing)
    } else {
        Ok(specs)
    }
}

fn build_surface(
    entities: &Vec<Entity>,
    surfel_specs_by_material_name: &HashMap<String, SurfelSpec>,
    unique_substance_names: &Vec<String>,
    surfel_distance: f32,
) -> Surface<Surfel<Vertex, SurfelData>> {
    let catchall_surfel_spec = surfel_specs_by_material_name.get("_");
    let default_substance_concentration = 0.0;
    let default_deposition_rate = 0.0;

    entities
        .iter()
        .enumerate()
        .fold(
            SurfaceBuilder::new()
                // TODO make this configurable
                .sampling(SurfelSampling::MinimumDistance(surfel_distance)),
            |b, (entity_idx, ent)| {
                let material_name = ent.material.name();

                let surfel_spec = surfel_specs_by_material_name
                    .get(material_name)
                    .or(catchall_surfel_spec);

                if let Some(surfel_spec) = surfel_spec {
                    let rules = surfel_spec
                        .rules
                        .iter()
                        .map(|r| rule_by_spec(r, &unique_substance_names))
                        .collect();

                    let proto_surfel = SurfelData {
                        entity_idx,
                        delta_straight: surfel_spec.reflectance.delta_straight,
                        delta_parabolic: surfel_spec.reflectance.delta_parabolic,
                        delta_flow: surfel_spec.reflectance.delta_flow,
                        substances: extract_keys(
                            &surfel_spec.initial,
                            &unique_substance_names,
                            default_substance_concentration,
                        ),
                        /// Weights for the transport of substances from a settled ton to a surfel
                        deposition_rates: extract_keys(
                            &surfel_spec.deposit,
                            &unique_substance_names,
                            default_deposition_rate,
                        ),
                        rules,
                    };

                    info!(
                        "Sampling entity \"{}\" into surfel representation, 2r={}…",
                        ent.name, surfel_distance
                    );

                    b.sample_triangles(ent.mesh.triangles(), &proto_surfel)
                } else {
                    // If no surfel spec is defined in the YAML, ignore the entity for the simulation
                    b
                }
            },
        )
        .build()
}

fn rule_by_spec(spec: &SurfelRuleSpec, unique_substance_names: &[String]) -> SurfelRule {
    match spec {
        &SurfelRuleSpec::Transfer {
            ref from,
            ref to,
            factor,
        } => SurfelRule::Transfer {
            source_substance_idx: unique_substance_names
                .iter()
                .position(|n| n == from)
                .expect(&format!(
                    "Surfel transport rule references unknown substance name {}",
                    from
                )),
            target_substance_idx: unique_substance_names.iter().position(|n| n == to).expect(
                &format!(
                    "Surfel transport rule references unknown substance name {}",
                    to
                ),
            ),
            factor,
        },
        &SurfelRuleSpec::Deteriorate { ref from, factor } => SurfelRule::Deteriorate {
            substance_idx: unique_substance_names
                .iter()
                .position(|n| n == from)
                .expect(&format!(
                    "Surfel transport rule references unknown substance name {}",
                    from
                )),
            factor,
        },
        &SurfelRuleSpec::Deposit { ref to, amount } => SurfelRule::Deposit {
            substance_idx: unique_substance_names
                .iter()
                .position(|n| n == to)
                .expect(&format!(
                    "Surfel transport rule references unknown substance name {}",
                    to
                )),
            amount,
        },
    }
}

/// Extracts the values of the given vector of keys from the given map.
/// If no value is found under the given key, the given default is stored in its place.
fn extract_keys<K: Eq + Hash, V: Clone>(map: &HashMap<K, V>, keys: &Vec<K>, default: V) -> Vec<V> {
    keys.iter()
        .map(|k| map.get(k).unwrap_or(&default).clone())
        .collect()
}
