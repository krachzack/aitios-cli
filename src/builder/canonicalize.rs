use builder::{Error, ResolveErrorKind};
use files::Resolver;
use spec::{EffectSpec, SimulationSpec, Stop};
use std::collections::HashMap;
use std::path::PathBuf;

/// Makes relative paths in the spec fragment absolute using the given resolver.
///
/// Useful when combining specs with slightly different base paths for resolving.
pub fn canonicalize(
    mut spec: SimulationSpec,
    resolver: &Resolver,
) -> Result<SimulationSpec, Error> {
    resolve_scenes(&mut spec.scenes, resolver)?;
    resolve_ton_source_specs(&mut spec.sources, resolver)?;
    resolve_surfel_specs(&mut spec.surfels_by_material, resolver)?;
    resolve_effect_spec_paths(&mut spec.effects, resolver)?;
    // FIXME resolving outputs works differently
    // resolve_benchmarks(&mut spec.benchmark, resolver)?;
    Ok(spec)
}

fn resolve_scenes(scenes: &mut Vec<PathBuf>, resolver: &Resolver) -> Result<(), Error> {
    for scene in scenes.iter_mut() {
        *scene = resolver
            .resolve(&scene)
            .map_err(|e| Error::resolve(e, ResolveErrorKind::Scene))?;
    }

    Ok(())
}

fn resolve_ton_source_specs(
    source_spec_paths: &mut Vec<PathBuf>,
    resolver: &Resolver,
) -> Result<(), Error> {
    for spec in source_spec_paths.iter_mut() {
        *spec = resolver
            .resolve(&spec)
            .map_err(|e| Error::resolve(e, ResolveErrorKind::TonSourceSpec))?;
    }

    Ok(())
}

fn resolve_surfel_specs(
    surfels_by_material: &mut HashMap<String, String>,
    resolver: &Resolver,
) -> Result<(), Error> {
    for source_spec_path in surfels_by_material.values_mut() {
        *source_spec_path = resolver
            .resolve(&source_spec_path)
            .map(|p| p.to_str().unwrap().to_string())
            .map_err(|e| Error::resolve(e, ResolveErrorKind::TonSourceSpec))?
    }

    // FIXME dammit how do I resolve the contained mesh...

    Ok(())
}

fn resolve_effect_spec_paths(
    specs: &mut Vec<EffectSpec>,
    resolver: &Resolver,
) -> Result<(), Error> {
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
                    resolve_stop_list_paths(&mut normal.stops, resolver)?;
                }
                if let Some(displacement) = displacement {
                    resolve_stop_list_paths(&mut displacement.stops, resolver)?;
                }
                if let Some(albedo) = albedo {
                    resolve_stop_list_paths(&mut albedo.stops, resolver)?;
                }
                if let Some(metallicity) = metallicity {
                    resolve_stop_list_paths(&mut metallicity.stops, resolver)?;
                }
                if let Some(roughness) = roughness {
                    resolve_stop_list_paths(&mut roughness.stops, resolver)?;
                }
            }
            _ => (),
        }
    }
    Ok(())
}

fn resolve_stop_list_paths(stops: &mut Vec<Stop>, resolver: &Resolver) -> Result<(), Error> {
    for stop in stops.iter_mut() {
        stop.sample = if let Some(sample) = stop.sample.as_ref() {
            Some(
                resolver
                    .resolve(sample)
                    .map_err(|e| Error::resolve(e, ResolveErrorKind::Layer))?,
            )
        } else {
            None
        }
    }
    Ok(())
}

/*fn resolve_benchmarks(benches: &mut Option<BenchSpec>, resolver: &Resolver) -> Result<(), Error> {
    if let Some(spec) = benches.as_mut() {
        if let Some(iterations) = spec.iterations.as_mut() {
            *iterations = resolver
                .resolve(&iterations)
                .map_err(|e| Error::resolve(e, ResolveErrorKind::Benchmark))?;
        }
        if let Some(tracing) = spec.tracing.as_mut() {
            *tracing = resolver
                .resolve(&tracing)
                .map_err(|e| Error::resolve(e, ResolveErrorKind::Benchmark))?;
        }
        if let Some(synthesis) = spec.synthesis.as_mut() {
            *synthesis = resolver
                .resolve(&synthesis)
                .map_err(|e| Error::resolve(e, ResolveErrorKind::Benchmark))?;
        }
        if let Some(setup) = spec.setup.as_mut() {
            *setup = resolver
                .resolve(&setup)
                .map_err(|e| Error::resolve(e, ResolveErrorKind::Benchmark))?;
        }
    }
    Ok(())
}*/
