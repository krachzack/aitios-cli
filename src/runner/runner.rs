use spec::{SimulationSpec, EffectSpec, SurfelLookup};
use sim::Simulation;
use scene::{Entity, MaterialBuilder};
use tex::{self, Density, Rgba, build_surfel_lookup_table};
use std::io;
use std::fs::{File, create_dir_all};
use std::path::PathBuf;
use std::fmt;
use std::rc::Rc;
use std::collections::HashMap;
use asset::obj;
use chrono::prelude::*;

pub struct SimulationRunner {
    spec: SimulationSpec,
    iteration: usize,
    unique_substance_names: Vec<String>,
    sim: Simulation,
    entities: Vec<Entity>,
    /// Maps resolutions against precomputed surfel indexes per texel, per entity.
    surfel_tables: HashMap<(usize, usize), Vec<Vec<Vec<(f32, usize)>>>>,
    /// The local time at which the runner was created.
    creation_time: DateTime<Local>
}

impl SimulationRunner {
    pub fn new(spec: SimulationSpec, unique_substance_names: Vec<String>, sim: Simulation, entities: Vec<Entity>) -> Self {

        let mut surfel_tables = HashMap::new();

        for spec in &spec.effects {
            // REVIEW if multiple densities have different island_bleed, it will be overwritten
            match spec {
                &EffectSpec::Density {
                    width,
                    height,
                    island_bleed,
                    surfel_lookup,
                    ..
                } => {
                    info!("Preparing {}x{} surfel lookup tables for accelerated texture synthesis on static scenes.", width, height);
                    surfel_tables.entry((width, height))
                        .or_insert_with(|| entities.iter().enumerate().map(|(idx, e)| {
                            info!("{} surfel table is being assembled… ({}/{})", e.name, (idx+1), entities.len());
                            match surfel_lookup {
                                SurfelLookup::Nearest { count } =>
                                    build_surfel_lookup_table(e, sim.surface(), count, width, height, island_bleed),
                                SurfelLookup::Within { within: _within } =>
                                    unimplemented!()
                            }

                        }).collect());
                }
            }
        }

        SimulationRunner {
            iteration: 0, spec, unique_substance_names, sim, entities, surfel_tables,
            creation_time: Local::now()
        }
    }

    pub fn run(&mut self) {
        for iteration in 0..self.spec.iterations {
            self.iteration = iteration;
            info!("Iteration started… ({}/{})", (iteration + 1), self.spec.iterations);
            self.sim.run();
            for effect in &self.spec.effects {
                self.perform_effect(effect)
            }
        }
    }

    fn perform_effect(&self, effect: &EffectSpec) {
        match effect {
            &EffectSpec::Density {
                width,
                height,
                island_bleed,
                ref tex_pattern,
                ref obj_pattern,
                ref mtl_pattern,
                ..
            } => self.perform_density_effect(width, height, island_bleed, tex_pattern, obj_pattern, mtl_pattern)
        }
    }

    fn perform_density_effect(&self, width: usize, height: usize, island_bleed: usize, tex_pattern: &String, obj_pattern: &Option<String>, mtl_pattern: &Option<String>) {
        for substance_idx in 0..self.unique_substance_names.len() {
            let density = Density::new(
                substance_idx,
                width, // tex_width
                height, // tex_height
                island_bleed,
                0.0, // min_density
                1.0, // max_density
                Rgba { data: [ 255, 255, 255, 255 ] }, // undefined_color
                Rgba { data: [ 255, 255, 255, 255 ] }, // min color
                Rgba { data: [ 0, 0, 0, 255 ] } // max color
            );

            let mut entities_with_density_maps = Vec::new();
            let datetime = &self.creation_time.to_rfc3339()
                .replace(":", "_");

            for (idx, ent) in self.entities.iter().enumerate() {
                // {iteration}-{id}-{entity}-{substance}.png
                let tex_filename = tex_pattern.replace("{iteration}", &format!("{}", (1 + self.iteration)))
                    .replace("{id}", &format!("{}", idx))
                    .replace("{entity}", &ent.name)
                    .replace("{substance}", &self.unique_substance_names[substance_idx])
                    .replace("{datetime}", datetime);

                let ref mut fout = create_file_recursively(&tex_filename)
                    .expect("Could not create image file for density effect.");

                info!("Collecting density: {}", &tex_filename);
                let surfel_table_resolution = (width, height);
                let density_tex = density.collect_with_table(
                    self.sim.surface(),
                    &self.surfel_tables.get(&surfel_table_resolution).unwrap()[idx]
                );
                info!("Writing density: {}", &tex_filename);

                tex::ImageRgba8(density_tex)
                    .save(fout, tex::PNG)
                    .expect("Density texture could not be persisted");

                if obj_pattern.is_some() || mtl_pattern.is_some() {
                    entities_with_density_maps.push(Entity {
                        name: ent.name.clone(),
                        mesh: ent.mesh.clone(),
                        material: Rc::new(MaterialBuilder::new()
                            .name(format!("{}-density-{}-{}", self.unique_substance_names[substance_idx], idx, ent.name))
                            .diffuse_color_map(tex_filename)
                            .build())
                    });
                }

            }

            match (obj_pattern, mtl_pattern) {
                (&Some(ref obj_pattern), &Some(ref mtl_pattern)) => {
                    let obj_filename = obj_pattern.replace("{iteration}", &format!("{}", (1 + self.iteration)))
                        .replace("{substance}", &self.unique_substance_names[substance_idx])
                        .replace("{datetime}", datetime);

                    let mtl_filename = mtl_pattern.replace("{iteration}", &format!("{}", (1 + self.iteration)))
                        .replace("{substance}", &self.unique_substance_names[substance_idx])
                        .replace("{datetime}", datetime);

                    info!("Persisting scene: {}", obj_filename);

                    create_file_recursively(&obj_filename)
                        .expect("Failed to create OBJ file when persisting effect results");
                    create_file_recursively(&mtl_filename)
                        .expect("Failed to create MTL file when persisting effect results");

                    obj::save(entities_with_density_maps.iter(), Some(obj_filename), Some(mtl_filename))
                        .expect("Failed to save OBJ/MTL");
                }
                _ => unimplemented!("Only combined OBJ/MTL output supported by now")
            }
        }
    }
}

/// Attempts to create or overwrite the file at the given path.
///
/// If it exists and is a file, it will be overwritten.
///
/// If it exists and is a directory or some other non-file entity,
/// an error of kind `io::ErrorKind::InvalidData` is returned.
///
/// If the directory does not exist, the function attempts to create
/// intermediate directories necessary to create it and finally
/// creates and returns the file.
fn create_file_recursively<P>(path: P) -> Result<File, io::Error>
    where P : Into<PathBuf>
{
    match &path.into() {
        // Path, following symlinks, already exists and is a file, overwrite it
        path if path.is_file() => File::create(&path),
        // Path, following symlinks, already exists and is a directory, fail with specific error message
        path if path.is_dir() => Err(
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Tried to create a file at {}, but a directory already exists at the same path.",
                    path.to_str().unwrap_or("NON-UTF-8")
                )
            )
        ),
        // Some entity exists at path, following symlinks, but it is neither a directory,
        // nor a file. Do not attempt to overwrite and instead fail with generic error message.
        path if path.exists() => Err(
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Tried to create a file at {}, but some entity that is neither a file nor a directory already exists at the same path.",
                    path.to_str().unwrap_or("NON-UTF-8")
                )
            )
        ),
        // Nothing exists at path yet, try to create intermediate directories and the file itself.
        path => {
            if let Some(parent) = path.parent() {
                create_dir_all(parent)?
            }

            File::create(&path)
        }
    }
}

impl fmt::Display for SimulationRunner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Name:               {}\n", self.spec.name)?;
        write!(f, "Description:        {}\n", self.spec.description)?;
        write!(f, "Scene:              {}\n", self.spec.scene.file_name().unwrap().to_str().unwrap())?;
        write!(f, "Iterations:         {}\n", self.spec.iterations)?;
        write!(f, "Surfels:            {}\n", self.sim.surfel_count())?;
        write!(f, "Tons per iteration: {}\n", self.sim.emission_count())?;
        write!(f, "Substances:         {:?}", self.unique_substance_names)
    }
}
