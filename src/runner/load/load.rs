use runner::SimulationRunner;
use geom::{Vec3, Vertex};
use asset::obj;
use scene::{Entity, Mesh};
use sim::{Simulation, SurfelData, SurfelRule, TonSourceBuilder, TonSource};
use spec::{SimulationSpec, SurfelSpec, SurfelRuleSpec, TonSourceSpec, EffectSpec, Stop};
use surf::{Surface, SurfaceBuilder, SurfelSampling, Surfel};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;
use std::cmp::Eq;
use std::hash::Hash;
use std::env::current_dir;
use serde_yaml;
use files::Resolver;
use failure::{Error, ResultExt};
use runner::load::err::LoadError;

pub fn load<P : Into<PathBuf>>(simulation_spec_file: P) -> Result<SimulationRunner, Error> {
    let simulation_spec_path = simulation_spec_file.into();

    if !simulation_spec_path.exists() {
        return Err(format_err!("Simulaction spec does not exist"));
    }

    let mut simulation_spec_file = File::open(&simulation_spec_path)
        .context("Simulation spec could not be opened.")?;

    let resolver = build_resolver(&simulation_spec_path)?;

    let mut spec : SimulationSpec = serde_yaml::from_reader(&mut simulation_spec_file)
        .context("Failed parsing simulation spec.")?;

    let surfel_specs_by_material_name = surfel_specs_by_material_name(&spec, &resolver)?;

    let entities = {
        let scene_path = resolver.resolve(&spec.scene)
            .context("Simulation scene could not be found.")?;
        let mut entities = obj::load(&scene_path)
            .context("Simulation scene was found but could not be read.")?;

        // Throw out all entitites which have no mapped surfel spec,
        // unless there is a fallback material named "_".
        // This ignoring affects intersection test and surfel generation,
        // potentially providing a massive speedup if many objects ignored.
        if !surfel_specs_by_material_name.contains_key("_") {
            entities.retain(|e|
                surfel_specs_by_material_name.keys()
                    .any(|n| n == e.material.name()));
        }

        entities
    };

    let source_specs = load_source_specs(&spec.sources, &resolver)?;

    let unique_substance_names = unique_substance_names(&surfel_specs_by_material_name, &source_specs);

    if unique_substance_names.is_empty() {
        return Err(From::from(LoadError::SubstancesMissing));
    }

    if spec.effects.is_empty() {
        return Err(From::from(LoadError::EffectsMissing));
    }

    resolve_effect_spec_paths(&mut spec.effects, &resolver)?;

    //let surfel_rules = build_surfel_rules(&surfel_specs_by_material_name, &unique_substance_names);
    let sources = build_sources(&source_specs, &unique_substance_names, &resolver)?;
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

/// Precedence:
/// 1. Absolute paths that do also exist
/// 2. Relative to directory that contains simulation spec
/// 3. Current working directory, if different from 2.
fn build_resolver(simulation_spec_path: &PathBuf) -> Result<Resolver, Error> {
    let mut resolver = Resolver::new();

    if let Some(spec_parent) = simulation_spec_path.parent() {
        if !spec_parent.as_os_str().is_empty() {
            resolver.add_base(spec_parent)?;
        }
    }

    resolver.add_base(
        current_dir()?
    )?;

    Ok(resolver)
}

/// For faster substance access, each substance name gets an ID which is an
/// index into the returned vector. Names can occur in sources, and surfels
/// as initial values and as absorption/deposition rates
fn unique_substance_names(
    surfel_specs: &HashMap<String, SurfelSpec>,
    source_specs: &Vec<TonSourceSpec>
) -> Vec<String>
{
    let unique_substance_names : HashSet<&String> = surfel_specs.values()
        .flat_map(|s| s.initial.keys().chain(s.deposit.keys()))
        .chain(source_specs.iter().flat_map(|s| s.initial.keys().chain(s.absorb.keys())))
        .collect();

    unique_substance_names.into_iter().cloned().collect()
}

fn resolve_effect_spec_paths(specs: &mut Vec<EffectSpec>, resolver: &Resolver) -> Result<(), Error> {
    for effect in specs.iter_mut() {
        match effect {
            EffectSpec::Layer {
                ref mut normal,
                ref mut displacement,
                ref mut albedo,
                ref mut metallicity,
                ref mut roughness,
                ..
             } => {
                 if let Some(normal) = normal {
                     resolve_stop_list_paths(&mut normal.stops, resolver)
                        .context("Layer normal blend effect references an image that could not be found.")?;
                 }
                 if let Some(displacement) = displacement {
                     resolve_stop_list_paths(&mut displacement.stops, resolver)
                        .context("Layer displacement blend effect references an image that could not be found.")?;
                 }
                 if let Some(albedo) = albedo {
                     resolve_stop_list_paths(&mut albedo.stops, resolver)
                        .context("Layer albedo blend effect references an image that could not be found.")?;
                 }
                 if let Some(metallicity) = metallicity {
                     resolve_stop_list_paths(&mut metallicity.stops, resolver)
                        .context("Layer metallicity blend effect references an image that could not be found.")?;
                 }
                 if let Some(roughness) = roughness {
                     resolve_stop_list_paths(&mut roughness.stops, resolver)
                        .context("Layer roughness blend effect references an image that could not be found.")?;
                 }
             },
            _ => ()
        }
    }
    Ok(())
}

fn resolve_stop_list_paths(stops: &mut Vec<Stop>, resolver: &Resolver) -> Result<(), Error> {
    for stop in stops.iter_mut() {
        stop.sample = if let Some(sample) = stop.sample.as_ref() {
            Some(resolver.resolve(sample)?)
        } else {
            None
        }
    }
    Ok(())
}

fn load_source_specs(sources: &Vec<PathBuf>, resolver: &Resolver) -> Result<Vec<TonSourceSpec>, Error> {
    if sources.is_empty() {
        return Err(From::from(LoadError::SourcesMissing));
    }

    sources.iter()
        .map(|s| load_source_spec(s, resolver))
        .collect()
}

fn load_source_spec(path: &PathBuf, resolver: &Resolver) -> Result<TonSourceSpec, Error> {
    let path = resolver.resolve(path)
        .context("Ton source spec file could not be found.")?;

    let spec_file = &mut File::open(path)
        .context("Ton source spec file could not be opened.")?;

    let spec : TonSourceSpec = serde_yaml::from_reader(spec_file)
        .context("Parse error for ton source spec.")?;

    Ok(spec)
}

fn build_sources(sources: &Vec<TonSourceSpec>, unique_substance_names: &Vec<String>, resolver: &Resolver) -> Result<Vec<TonSource>, Error> {
    sources.iter()
        .map(|spec| {
            let mesh = resolver.resolve(&spec.mesh)
                .context("Ton source emission mesh could not be resolved.")?;
            let mesh = &obj::load(&mesh)
                .context("Ton source emission mesh could not be deserialized.")?;
            // REVIEW a valid OBJ file without contents is conveivable, this should provide a better error message
            //        not sure if should check here or upstream when loading.
            //        more than one mesh should probably be merged
            let mesh = &mesh[0].mesh;

            let mut builder = TonSourceBuilder::new();

            if let Some(ref direction_arr) = spec.flow_direction {
                builder = builder.flow_direction_static(Vec3::new(direction_arr[0], direction_arr[1], direction_arr[2]));
            }

             let source = builder.mesh_shaped(mesh, spec.diffuse)
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

fn surfel_specs_by_material_name(spec: &SimulationSpec, resolver: &Resolver) -> Result<HashMap<String, SurfelSpec>, Error> {
    let mut specs = HashMap::with_capacity(spec.surfels_by_material.len());

    for (material_name, surfel_spec) in spec.surfels_by_material.iter() {
        let surfel_spec = &mut File::open(
            resolver.resolve(surfel_spec)
                .context("Surfel spec could not be found.")?
        )?;

        let surfel_spec : SurfelSpec = serde_yaml::from_reader(surfel_spec)
            .context("Surfel spec could not be parsed.")?;

        specs.insert(material_name.clone(), surfel_spec);
    }

    if specs.is_empty() {
        Err(From::from(LoadError::SurfelSpecsMissing))
    } else {
        Ok(specs)
    }
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
