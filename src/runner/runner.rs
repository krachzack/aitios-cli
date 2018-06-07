use asset::obj;
use sim::Simulation;
use scene::{Entity, MaterialBuilder};
use spec::{EffectSpec, SimulationSpec, SurfelLookup, Blend};
use tex::{Density, Rgba, GuidedBlend, Stop, DynamicImage};
use std::path::PathBuf;
use std::fmt;
use std::rc::Rc;
use chrono::prelude::*;
use files::create_file_recursively;
use runner::surfel_table_cache::SurfelTableCache;
use surf;
use tex::{self, GenericImage};
use geom::Vertex;
use sim::SurfelData;

type Surface = surf::Surface<surf::Surfel<Vertex, SurfelData>>;

pub struct SimulationRunner {
    spec: SimulationSpec,
    sim: Simulation,
    /// The local time at which the runner was created.
    creation_time: DateTime<Local>,
    iteration: u64,
    unique_substance_names: Vec<String>,
    entities: Vec<Entity>,
    surfel_tables: SurfelTableCache
}

impl SimulationRunner {
    pub fn new(spec: SimulationSpec, unique_substance_names: Vec<String>, sim: Simulation, entities: Vec<Entity>) -> Self {
        let creation_time = Local::now();

        let surfel_tables = build_surfel_tables(&spec.effects, &entities, sim.surface());

        Self {
            spec,
            sim,
            creation_time,
            iteration: 0,
            unique_substance_names,
            entities,
            surfel_tables
        }
    }

    pub fn run(&mut self) {
        // Iteration 0 only performs effects, no tracing is performed.
        // Useful as a reference for iteration 1
        self.iteration = 0;
        self.perform_effects();

        for _ in 0..self.spec.iterations {
            // iteration 1 is the first iteration with actual gammaton simulation before effects
            self.iteration += 1;
            self.perform_iteration();
        }
    }

    fn perform_iteration(&mut self) {
        // Perform tracing and substance transport
        self.sim.run();

        // NOTE surfel table cache invalidation necessary if geometry was changed
        self.perform_effects();
    }

    fn perform_effects(&self) {
        // Make a fresh copy of the scene to run the effects on for each effect run.
        // With this technique, effects can accumulate throughout one iteration,
        // but each iteration will start with a fresh copy of the scene.
        let mut entities = self.entities.clone();

        for effect in &self.spec.effects {
            self.perform_effect(effect, &mut entities);
        }
    }

    // Applies the given effect.
    fn perform_effect(&self, effect: &EffectSpec, entities: &mut Vec<Entity>) {
        match effect {
            &EffectSpec::Density {
                width,
                height,
                island_bleed,
                surfel_lookup,
                ref tex_pattern,
                ref obj_pattern,
                ref mtl_pattern,
                ..
            } => self.perform_density(width, height, island_bleed, surfel_lookup, tex_pattern, obj_pattern, mtl_pattern),
            &EffectSpec::DumpSurfels { ref obj_pattern } => self.export_surfels(obj_pattern),
            &EffectSpec::Layer {
                ref materials,
                ref substance,
                surfel_lookup,
                island_bleed,
                ref normal,
                ref displacement,
                ref albedo,
                ref metallicity,
                ref roughness
            } => self.perform_layer(
                entities,
                materials,
                substance,
                surfel_lookup,
                island_bleed,
                normal,
                displacement,
                albedo,
                metallicity,
                roughness
            ),
            &EffectSpec::Export {
                ref obj_pattern,
                ref mtl_pattern
            } => self.export_scene(entities.iter(), obj_pattern, mtl_pattern, "all") // When {substance} is used, write "all"
        }
    }

    /// For each substance, create a density map for each entity, then serialize a scene with
    /// textures applied. Does not influence other effects and leaves the original scene unchanged.
    /// Useful for debugging.
    fn perform_density(&self, width: usize, height: usize, island_bleed: usize, surfel_lookup: SurfelLookup, tex_pattern: &String, obj_pattern: &Option<String>, mtl_pattern: &Option<String>) {
        for (substance_idx, substance_name) in self.unique_substance_names.iter().enumerate() {
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

            // Make lazy copy of original scene with each material replaced
            // by a new one with diffuse color set to substance density
            let density_scene = self.entities.iter()
                .enumerate()
                .map(|(ent_idx, ent)| {
                    let surfel_table = self.surfel_tables.lookup(
                        ent_idx,
                        width,
                        height,
                        surfel_lookup,
                        island_bleed
                    );

                    let density_tex = density.collect_with_table(
                        self.sim.surface(),
                        surfel_table
                    );

                    let tex_filename = tex_pattern.replace("{iteration}", &format!("{}", self.iteration))
                        .replace("{id}", &format!("{}", ent_idx))
                        .replace("{entity}", &ent.name)
                        .replace("{substance}", substance_name)
                        .replace("{datetime}", &self.creation_time.to_rfc3339().replace(":", "_"));

                    let mut fout = create_file_recursively(&tex_filename)
                        .expect("Could not create image file for density effect.");

                    tex::ImageRgba8(density_tex)
                        .write_to(&mut fout, tex::PNG)
                        .expect("Density texture could not be persisted");

                    // Reference old entity name and mesh, but replace
                    // material in a fresh entity
                    Entity {
                        material: Rc::new(
                            MaterialBuilder::new()
                                .name(format!("{}-density-{}-{}", self.unique_substance_names[substance_idx], ent_idx, ent.name))
                                .diffuse_color_map(tex_filename)
                                .build()
                        ),
                        ..ent.clone()
                    }
                })
                .collect::<Vec<_>>();

            self.export_scene(density_scene.iter(), obj_pattern, mtl_pattern, &substance_name);
        }
    }

    fn perform_layer(
        &self,
        entities: &mut Vec<Entity>,
        materials: &Vec<String>,
        substance: &String,
        surfel_lookup: SurfelLookup,
        island_bleed: usize,
        // REVIEW should normal and displacement be usable together? maybe the normal map should be derived from the displacement map to ensure consistency
        normal: &Option<Blend>,
        displacement: &Option<Blend>,
        albedo: &Option<Blend>,
        metallicity: &Option<Blend>,
        roughness: &Option<Blend>
    )
    {
        let substance_idx = self.unique_substance_names
            .iter()
            .position(|s| s == substance)
            .expect(&format!("Blend substance does not exist: {}", substance));

        entities.iter_mut()
            .enumerate()
            .filter(|(_, e)| materials.iter().any(|m| m == e.material.name()))
            .for_each(|(idx, entity)| {
                let mut mat = MaterialBuilder::from(&*entity.material);

                if let Some(_normal) = normal { unimplemented!("No algorithm for blending normals implemented yet") }

                if let Some(displacement) = displacement {
                    let new_tex_path = self.perform_blend(entity, entity.material.displacement_map(), displacement, substance_idx, idx, surfel_lookup, island_bleed);
                    mat = mat.displacement_map(new_tex_path);
                }

                if let Some(albedo) = albedo {
                    let new_tex_path = self.perform_blend(entity, entity.material.diffuse_color_map(), albedo, substance_idx, idx, surfel_lookup, island_bleed);
                    mat = mat.diffuse_color_map(new_tex_path);
                }

                if let Some(metallicity) = metallicity {
                    let new_tex_path = self.perform_blend(entity, entity.material.metallic_map(), metallicity, substance_idx, idx, surfel_lookup, island_bleed);
                    mat = mat.metallic_map(new_tex_path);
                }

                if let Some(roughness) = roughness {
                    let new_tex_path = self.perform_blend(entity, entity.material.roughness_map(), roughness, substance_idx, idx, surfel_lookup, island_bleed);
                    mat = mat.roughness_map(new_tex_path);
                }

                entity.material = Rc::new(mat.build());
            });

    }

    fn perform_blend(&self, entity: &Entity, original_map: Option<&PathBuf>, blend: &Blend, substance_idx: usize, entity_idx: usize, surfel_lookup: SurfelLookup, island_bleed: usize) -> PathBuf {
        let (width, height) = blend_output_size(
            blend,
            original_map
        );

        let table = self.surfel_tables.lookup(
            entity_idx,
            width as usize,
            height as usize,
            surfel_lookup,
            island_bleed
        );

        let guide = Density::new(
            substance_idx,
            width as usize, // tex_width
            height as usize, // tex_height
            island_bleed,
            0.0, // min_density
            1.0, // max_density
            Rgba { data: [ 0,   0,   0,   255 ] }, // undefined_color
            Rgba { data: [ 0,   0,   0,   255 ] }, // min color
            Rgba { data: [ 255, 255, 255, 255 ] } // max color
        ).collect_with_table(
            self.sim.surface(),
            table
        );

        let guided_blend = Self::make_guided_blend(blend, original_map);
        let blend_result_tex = guided_blend.perform(&guide);

        let tex_filename = blend.tex_pattern
            .replace("{iteration}", &format!("{}", self.iteration))
            .replace("{id}", &format!("{}", entity_idx))
            .replace("{entity}", &entity.name)
            .replace("{substance}", &self.unique_substance_names[substance_idx])
            .replace("{datetime}", &self.creation_time.to_rfc3339().replace(":", "_"));

        let mut tex_file = create_file_recursively(&tex_filename)
            .expect("Could not create texture file for blending effect");

        tex::ImageRgba8(blend_result_tex)
            .write_to(&mut tex_file, tex::PNG)
            .expect("Density texture could not be persisted");

        PathBuf::from(tex_filename)
    }

    fn make_guided_blend(blend: &Blend, original_map: Option<&PathBuf>) -> GuidedBlend<DynamicImage> {
        let mut stops = Vec::with_capacity(blend.stops.len() + 1);

        // Add implicit 0.0 stop with original texture, if present
        match original_map {
            Some(original_map) => if !blend.stops.iter().any(|s| s.cenith == 0.0) {
                stops.push(Stop::new(0.0, tex::open(original_map).unwrap()));
            }
            None => if blend.stops.is_empty() {
                panic!("Failed to do a blend effect because no stops are defined and no original map is defined either")
            }
        }

        // Then add the configured stops
        for stop in &blend.stops {
            stops.push(
                Stop::new(
                    stop.cenith,
                    tex::open(
                        stop.sample.as_ref().or(original_map)
                            .expect("Defined a blend stop without texture, but applicable material does not define base texture")
                    ).expect("Blend stop texture could not be loaded")
                )
            )
        }

        GuidedBlend::new(stops.into_iter())
    }

    fn export_scene<'a, E>(&'a self, entities: E, obj_pattern: &Option<String>, mtl_pattern: &Option<String>, substance: &str)
        where E : IntoIterator<Item = &'a Entity>
    {
        let datetime = &self.creation_time.to_rfc3339()
                .replace(":", "_");

        // TODO handle deduplication of material names,
        // e.g. group by name and then make every multiply used name unique if values differ

        match (obj_pattern, mtl_pattern) {
            (&Some(ref obj_pattern), &Some(ref mtl_pattern)) => {
                let obj_filename = obj_pattern.replace("{iteration}", &format!("{}", self.iteration))
                    .replace("{substance}", substance)
                    .replace("{datetime}", datetime);

                let mtl_filename = mtl_pattern.replace("{iteration}", &format!("{}", self.iteration))
                    .replace("{substance}", substance)
                    .replace("{datetime}", datetime);

                info!("Persisting scene: {}", obj_filename);

                create_file_recursively(&obj_filename)
                    .expect("Failed to create OBJ file when persisting effect results.");
                create_file_recursively(&mtl_filename)
                    .expect("Failed to create MTL file when persisting effect results.");

                obj::save(entities, Some(obj_filename), Some(mtl_filename))
                    .expect("Failed to save OBJ/MTL.");
            },
            (&None, &None) => (),
            _ => unimplemented!("Individual OBJ/MTL output without its counterpart unsupported by now. Export counterpart too to make it work.")
        }
    }

    fn export_surfels(&self, surfel_obj_pattern: &str) {
        let datetime = &self.creation_time.to_rfc3339()
                .replace(":", "_");

        let surfel_obj_path = surfel_obj_pattern.replace("{iteration}", &format!("{}", self.iteration))
            .replace("{datetime}", datetime);

        let mut obj_file = create_file_recursively(surfel_obj_path)
            .expect("Failed to create OBJ file to save surfels into.");

        self.sim.surface()
            .dump(&mut obj_file)
            .expect("Failed to save surfels to OBJ file");
    }
}

fn build_surfel_tables(effects: &Vec<EffectSpec>, entities: &Vec<Entity>, surface: &Surface) -> SurfelTableCache {
    let mut surfel_tables = SurfelTableCache::new();

    for effect in effects {
        match effect {
            &EffectSpec::Layer {
                island_bleed,
                surfel_lookup,
                ref materials,
                ref normal,
                ref displacement,
                ref albedo,
                ref metallicity,
                ref roughness,
                ..
            } => entities.iter()
                .enumerate()
                // Ignore entities with a material not affected by this synthesis
                .filter(|(_, e)| materials.iter().any(|m| m == e.material.name()))
                // And cache
                .for_each(|(idx, e)| {
                    let material = &e.material;

                    if let Some(normal) = normal {
                        let (width, height) = blend_output_size(
                            normal,
                            material.normal_map()
                        );

                        surfel_tables.prepare(
                            idx,
                            width as usize,
                            height as usize,
                            surfel_lookup,
                            island_bleed,
                            entities,
                            surface
                        )
                    }

                    if let Some(displacement) = displacement {
                        let (width, height) = blend_output_size(
                            displacement,
                            material.displacement_map()
                        );

                        surfel_tables.prepare(
                            idx,
                            width as usize,
                            height as usize,
                            surfel_lookup,
                            island_bleed,
                            entities,
                            surface
                        )
                    }

                    if let Some(albedo) = albedo {
                        let (width, height) = blend_output_size(
                            albedo,
                            material.diffuse_color_map()
                        );

                        surfel_tables.prepare(
                            idx,
                            width as usize,
                            height as usize,
                            surfel_lookup,
                            island_bleed,
                            entities,
                            surface
                        )
                    }

                    if let Some(metallicity) = metallicity {
                        let (width, height) = blend_output_size(
                            metallicity,
                            material.metallic_map()
                        );

                        surfel_tables.prepare(
                            idx,
                            width as usize,
                            height as usize,
                            surfel_lookup,
                            island_bleed,
                            entities,
                            surface
                        )
                    }

                    if let Some(roughness) = roughness {
                        let (width, height) = blend_output_size(
                            roughness,
                            material.roughness_map()
                        );

                        surfel_tables.prepare(
                            idx,
                            width as usize,
                            height as usize,
                            surfel_lookup,
                            island_bleed,
                            entities,
                            surface
                        )
                    }

                }),
            &EffectSpec::Density {
                width,
                height,
                island_bleed,
                surfel_lookup,
                ..
            } => (0..entities.len())
                .for_each(|idx| surfel_tables.prepare(
                    idx,
                    width,
                    height,
                    surfel_lookup,
                    island_bleed,
                    &entities,
                    surface
                )),
            _ => ()
        }
    }

    surfel_tables
}

fn blend_output_size(blend: &Blend, original_tex_path: Option<&PathBuf>) -> (u32, u32) {
    original_tex_path
        // Let diffuse color texture map determine surfel table resolution
        .map(|p| tex::open(p).expect(&format!("Texture of entity could not be loaded {:?}", p)))
        // If undefined, pick largest blending stop
        .or_else(|| blend.stops
            .iter()
            .filter_map(|s| s.sample.as_ref())
            .map(|p| tex::open(p).expect(&format!("Blend sample texture could not be loaded {:?}", p)))
            .max_by_key(|i| i.dimensions())
        )
        .map(|i| i.dimensions())
        .expect("Cannot determine surfel table size for layer effect. Neither the material nor any blend stop define a loadable texture that could be used to derive a size.")
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

