use spec::{SimulationSpec, EffectSpec};
use sim::Simulation;
use scene::{Entity, MaterialBuilder};
use tex::{self, Density, Rgba, build_surfel_lookup_table};
use std::fs::File;
use std::fmt;
use std::rc::Rc;
use std::collections::HashMap;
use asset::obj;

pub struct SimulationRunner {
    spec: SimulationSpec,
    iteration: usize,
    unique_substance_names: Vec<String>,
    sim: Simulation,
    entities: Vec<Entity>,
    /// Maps resolutions against precomputed surfel indexes per texel, per entity
    surfel_tables: HashMap<(usize, usize), Vec<Vec<Vec<(f32, usize)>>>>
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
                    ..
                } => {
                    info!("Preparing {}x{} surfel lookup tables for accelerated texture synthesis on static scenes.", width, height);
                    surfel_tables.entry((width, height))
                        .or_insert_with(|| entities.iter().enumerate().map(|(idx, e)| {
                            info!("{} surfel table is being assembled… ({}/{})", e.name, (idx+1), entities.len());
                            build_surfel_lookup_table(e, sim.surface(), 4, width, height, island_bleed)
                        }).collect());
                }
            }
        }

        SimulationRunner {
            iteration: 0, spec, unique_substance_names, sim, entities, surfel_tables
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
                ref mtl_pattern
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

            for (idx, ent) in self.entities.iter().enumerate() {
                // {iteration}-{id}-{entity}-{substance}.png
                let tex_filename = tex_pattern.replace("{iteration}", &format!("{}", self.iteration))
                    .replace("{id}", &format!("{}", idx))
                    .replace("{entity}", &ent.name)
                    .replace("{substance}", &self.unique_substance_names[substance_idx]);

                let ref mut fout = File::create(&tex_filename)
                    .expect(&format!("Could not create output file for density effect: \"{}\"", &tex_filename));

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
                    let obj_filename = obj_pattern.replace("{iteration}", &format!("{}", self.iteration))
                        .replace("{substance}", &self.unique_substance_names[substance_idx]);

                    let mtl_filename = mtl_pattern.replace("{iteration}", &format!("{}", self.iteration))
                        .replace("{substance}", &self.unique_substance_names[substance_idx]);

                    info!("Persisting scene: {}", obj_filename);
                    obj::save(entities_with_density_maps.iter(), Some(obj_filename), Some(mtl_filename))
                        .expect("Failed to save OBJ/MTL");
                }
                _ => unimplemented!("Only combined OBJ/MTL output supported by now")
            }
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
