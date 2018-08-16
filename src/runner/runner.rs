use asset::obj;
use bencher::Bencher;
use files::create_file_recursively;
use geom::Vertex;
use runner::surfel_table_cache::SurfelTableCache;
use scene::{Entity, MaterialBuilder};
use sim::Simulation;
use sim::SurfelData;
use spec::{BenchSpec, Blend, EffectSpec, SimulationSpec, SurfelLookup};
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use surf;
use tex::{self, GenericImage};
use tex::{
    combine_normals, open, BlendType, Density, DynamicImage, FilterType, GuidedBlend, Pixel, Rgba,
    Stop,
};

type Surface = surf::Surface<surf::Surfel<Vertex, SurfelData>>;

pub struct SimulationRunner {
    spec: SimulationSpec,
    sim: Simulation,
    iteration: u32,
    unique_substance_names: Vec<String>,
    entities: Vec<Entity>,
    surfel_tables: SurfelTableCache,
    iteration_benchmark: Option<Bencher>,
    tracing_benchmark: Option<Bencher>,
    synthesis_benchmark: Option<Bencher>,
    datetime: String,
}

impl SimulationRunner {
    pub fn new(
        spec: SimulationSpec,
        unique_substance_names: Vec<String>,
        sim: Simulation,
        entities: Vec<Entity>,
        // Datetime to replace in file patterns
        datetime: &str,
    ) -> Self {
        let surfel_tables = build_surfel_tables(&spec.effects, &entities, sim.surface());

        let (iteration_benchmark, tracing_benchmark, synthesis_benchmark) =
            build_benchmarks(&spec.benchmark, datetime);

        Self {
            spec,
            sim,
            iteration: 0,
            unique_substance_names,
            entities,
            surfel_tables,
            iteration_benchmark,
            tracing_benchmark,
            synthesis_benchmark,
            datetime: String::from(datetime),
        }
    }

    pub fn spec(&self) -> &SimulationSpec {
        &self.spec
    }

    pub fn run(&mut self) {
        // Iteration 0 only performs effects, no tracing is performed.
        // Useful as a reference for iteration 1.
        self.iteration = 0;
        self.perform_effects();

        for _ in 0..self.iterations() {
            // Iteration 1 is the first iteration with actual gammaton simulation before effects.
            self.iteration += 1;
            self.perform_iteration();
        }
    }

    fn iterations(&self) -> u32 {
        // Default to 1 iteration
        self.spec.iterations.unwrap_or(1)
    }

    fn perform_iteration(&mut self) {
        // Write timings of complete iterations to CSV benchmarks if required
        // by simulation spec.
        let _iteration_bench = self.iteration_benchmark.as_ref().map(|b| b.bench());

        info!(
            "Iteration {} of {} started...",
            self.iteration,
            self.iterations()
        );

        // Perform tracing and substance transport every iteration.
        {
            let _tracing_and_transport_bench = self.tracing_benchmark.as_ref().map(|b| b.bench());

            info!("Tracing...");
            self.sim.run();
        }

        let effects_scheduled = match self.spec.effect_interval {
            // Interval is defined, 1-based iteration index must be divisible.
            Some(interval) if (self.iteration % interval) == 0 => true,
            // Either no interval defined or defined and not divisible, skip effects,
            // except for the last iteration.
            _ => self.iteration == self.iterations(),
        };

        if effects_scheduled {
            // NOTE surfel table cache invalidation necessary if geometry was changed
            info!("Texture synthesis...");
            self.perform_effects();
        }
    }

    fn perform_effects(&self) {
        // NOTE this will run for iteration 0, so there will be one benchmark more for
        //      synthesis when compared to tracing
        let _synthesis_bench = self.synthesis_benchmark.as_ref().map(|b| b.bench());

        // Make a fresh copy of the scene to run the effects on for each effect run.
        // With this technique, effects can accumulate throughout one iteration,
        // but each iteration will apply its effects on top of the base material.
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
            } => self.perform_density(
                width,
                height,
                island_bleed,
                surfel_lookup,
                tex_pattern,
                obj_pattern,
                mtl_pattern,
            ),
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
                ref roughness,
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
                roughness,
            ),
            &EffectSpec::Export {
                ref obj_pattern,
                ref mtl_pattern,
            } => self.export_scene(entities.iter(), obj_pattern, mtl_pattern, "all"), // When {substance} is used, write "all"
        }
    }

    /// For each substance, create a density map for each entity, then serialize a scene with
    /// textures applied. Does not influence other effects and leaves the original scene unchanged.
    /// Useful for debugging.
    fn perform_density(
        &self,
        width: usize,
        height: usize,
        island_bleed: usize,
        surfel_lookup: SurfelLookup,
        tex_pattern: &String,
        obj_pattern: &Option<String>,
        mtl_pattern: &Option<String>,
    ) {
        for (substance_idx, substance_name) in self.unique_substance_names.iter().enumerate() {
            let density = Density::new(
                substance_idx,
                width,  // tex_width
                height, // tex_height
                island_bleed,
                0.0, // min_density
                1.0, // max_density
                Rgba {
                    data: [255, 255, 255, 255],
                }, // undefined_color
                Rgba {
                    data: [255, 255, 255, 255],
                }, // min color
                Rgba {
                    data: [0, 0, 0, 255],
                }, // max color
            );

            // Make lazy copy of original scene with each material replaced
            // by a new one with diffuse color set to substance density
            let density_scene = self
                .entities
                .iter()
                .enumerate()
                .map(|(ent_idx, ent)| {
                    let surfel_table = self.surfel_tables.lookup(
                        ent_idx,
                        width,
                        height,
                        surfel_lookup,
                        island_bleed,
                    );

                    let density_tex = density.collect_with_table(self.sim.surface(), surfel_table);

                    let tex_filename = tex_pattern
                        .replace("{iteration}", &format!("{}", self.iteration))
                        .replace("{id}", &format!("{}", ent_idx))
                        .replace("{entity}", &ent.name)
                        .replace("{substance}", substance_name)
                        .replace("{datetime}", &self.datetime);

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
                                .name(format!(
                                    "{}-density-{}-{}",
                                    self.unique_substance_names[substance_idx], ent_idx, ent.name
                                ))
                                .diffuse_color_map(tex_filename)
                                .build(),
                        ),
                        ..ent.clone()
                    }
                })
                .collect::<Vec<_>>();

            self.export_scene(
                density_scene.iter(),
                obj_pattern,
                mtl_pattern,
                &substance_name,
            );
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
        roughness: &Option<Blend>,
    ) {
        let substance_idx = self
            .unique_substance_names
            .iter()
            .position(|s| s == substance)
            .expect(&format!("Blend substance does not exist: {}", substance));

        entities
            .iter_mut()
            .enumerate()
            .filter(|(_, e)| is_entity_applicable_for_materials(e, materials))
            .for_each(|(idx, entity)| {
                let mut mat = MaterialBuilder::from(&*entity.material);

                if let Some(normal) = normal {
                    let new_tex_path = self.perform_blend(
                        entity,
                        entity.material.normal_map(),
                        normal,
                        substance_idx,
                        idx,
                        surfel_lookup,
                        island_bleed,
                        BlendType::Normal,
                    );
                    mat = mat.normal_map(new_tex_path);
                }

                if let Some(displacement) = displacement {
                    let new_tex_path = self.perform_blend(
                        entity,
                        entity.material.displacement_map(),
                        displacement,
                        substance_idx,
                        idx,
                        surfel_lookup,
                        island_bleed,
                        BlendType::Linear,
                    );
                    mat = mat.displacement_map(new_tex_path);
                }

                if let Some(albedo) = albedo {
                    let new_tex_path = self.perform_blend(
                        entity,
                        entity.material.diffuse_color_map(),
                        albedo,
                        substance_idx,
                        idx,
                        surfel_lookup,
                        island_bleed,
                        BlendType::Linear,
                    );
                    mat = mat.diffuse_color_map(new_tex_path);
                }

                if let Some(metallicity) = metallicity {
                    let new_tex_path = self.perform_blend(
                        entity,
                        entity.material.metallic_map(),
                        metallicity,
                        substance_idx,
                        idx,
                        surfel_lookup,
                        island_bleed,
                        BlendType::Linear,
                    );
                    mat = mat.metallic_map(new_tex_path);
                }

                // REVIEW since mtl supports glossiness, maybe invert the roughness with a MTL filter
                if let Some(roughness) = roughness {
                    let new_tex_path = self.perform_blend(
                        entity,
                        entity.material.roughness_map(),
                        roughness,
                        substance_idx,
                        idx,
                        surfel_lookup,
                        island_bleed,
                        BlendType::Linear,
                    );
                    mat = mat.roughness_map(new_tex_path);
                }

                entity.material = Rc::new(mat.build());
            });
    }

    fn perform_blend(
        &self,
        entity: &Entity,
        original_map: Option<&PathBuf>,
        blend: &Blend,
        substance_idx: usize,
        entity_idx: usize,
        surfel_lookup: SurfelLookup,
        island_bleed: usize,
        blend_type: BlendType,
    ) -> PathBuf {
        let (width, height) = blend_output_size(blend, original_map);

        let table = self.surfel_tables.lookup(
            entity_idx,
            width as usize,
            height as usize,
            surfel_lookup,
            island_bleed,
        );

        let guide = Density::new(
            substance_idx,
            width as usize,  // tex_width
            height as usize, // tex_height
            island_bleed,
            0.0, // min_density
            1.0, // max_density
            Rgba {
                data: [0, 0, 0, 255],
            }, // undefined_color
            Rgba {
                data: [0, 0, 0, 255],
            }, // min color
            Rgba {
                data: [255, 255, 255, 255],
            }, // max color
        ).collect_with_table(self.sim.surface(), table);

        let guided_blend = Self::make_guided_blend(blend, blend_type, original_map);
        let mut blend_result_tex = guided_blend.perform(&guide);

        // If original map is specified, blend the synthesized
        // weathering signs over the original map.
        // If no original texture, keep the output map with transparency
        // without blending over.
        if let Some(original_map) = original_map {
            let mut original_map = open(original_map).unwrap();

            if blend_result_tex.dimensions() != original_map.dimensions() {
                let (width, height) = blend_result_tex.dimensions();
                original_map = original_map.resize(width, height, FilterType::Triangle);
            }

            assert_eq!(
                blend_result_tex.dimensions(),
                original_map.dimensions(),
                "When original map is present, result of layer blend should have same dimensions"
            );

            match blend_type {
                // For normals, add blended map to base map as detail normal map
                BlendType::Normal => blend_result_tex
                    .pixels_mut()
                    .zip(original_map.pixels())
                    .for_each(|(top, (_, _, bottom))| {
                        // TODO implement influence (maybe rotate top towards base?)
                        *top = combine_normals(bottom, *top);
                    }),
                // For albedo, roughness, etc modulate alpha with influence and blend over original
                BlendType::Linear => blend_result_tex
                    .pixels_mut()
                    .zip(original_map.pixels())
                    .for_each(|(top, (_, _, bottom))| {
                        let mut bottom = bottom.clone();
                        // Reduce alpha of top according to influence
                        if blend.influence != 1.0 {
                            top.apply_with_alpha(|c| c, |a| (((a as f32) * blend.influence) as u8));
                        }
                        bottom.blend(top);
                        *top = bottom;
                    }), // TODO maybe displacement needs some special treatment so the baseline is at 0.5
                        //      displacement and normals should maybe also be mutually exclusive
            }
        }

        let tex_filename = blend
            .tex_pattern
            .replace("{iteration}", &format!("{}", self.iteration))
            .replace("{id}", &format!("{}", entity_idx))
            .replace("{entity}", &entity.name)
            .replace("{substance}", &self.unique_substance_names[substance_idx])
            .replace("{datetime}", &self.datetime);

        let mut tex_file = create_file_recursively(&tex_filename)
            .expect("Could not create texture file for blending effect");

        tex::ImageRgba8(blend_result_tex)
            .write_to(&mut tex_file, tex::PNG)
            .expect("Density texture could not be persisted");

        PathBuf::from(tex_filename)
    }

    fn make_guided_blend(
        blend: &Blend,
        blend_type: BlendType,
        original_map: Option<&PathBuf>,
    ) -> GuidedBlend<DynamicImage> {
        let mut stops = Vec::with_capacity(blend.stops.len() + 1);

        // Add implicit 0.0 stop with original texture, if present
        match original_map {
            Some(original_map) => if !blend.stops.iter().any(|s| s.cenith == 0.0) {
                stops.push(Stop::new(0.0, tex::open(original_map).unwrap()));
            },
            None => if blend.stops.is_empty() {
                panic!("Failed to do a blend effect because no stops are defined and no original map is defined either")
            },
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

        GuidedBlend::with_type(stops.into_iter(), blend_type)
    }

    fn export_scene<'a, E>(
        &'a self,
        entities: E,
        obj_pattern: &Option<String>,
        mtl_pattern: &Option<String>,
        substance: &str,
    ) where
        E: IntoIterator<Item = &'a Entity>,
    {
        let datetime = &self.datetime;

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
        let datetime = &self.datetime;

        let surfel_obj_path = surfel_obj_pattern
            .replace("{iteration}", &format!("{}", self.iteration))
            .replace("{datetime}", datetime);

        let mut obj_file = create_file_recursively(surfel_obj_path)
            .expect("Failed to create OBJ file to save surfels into.");

        self.sim
            .surface()
            .dump(&mut obj_file)
            .expect("Failed to save surfels to OBJ file");
    }
}

// Underscore material is catchall as always, empty array also means admit all materials
fn is_entity_applicable_for_materials(entity: &Entity, materials: &Vec<String>) -> bool {
    materials.is_empty()
        || materials
            .iter()
            .any(|m| m == "_" || m == entity.material.name())
}

fn build_benchmarks(
    benchmark: &Option<BenchSpec>,
    creation_time: &str,
) -> (Option<Bencher>, Option<Bencher>, Option<Bencher>) {
    fn build_benchmark(target_file: &Option<PathBuf>, creation_time: &str) -> Option<Bencher> {
        target_file
            .as_ref()
            .and_then(|csv| {
                let csv = csv.to_str().unwrap().replace("{datetime}", creation_time);

                Some(create_file_recursively(csv).expect("Failed to create benchmark file"))
            })
            .and_then(|csv| Some(Bencher::new(csv)))
    }

    if let Some(ref benchmark) = benchmark {
        let iteration_benchmark = build_benchmark(&benchmark.iterations, creation_time);
        let tracing_benchmark = build_benchmark(&benchmark.tracing, creation_time);
        let synthesis_benchmark = build_benchmark(&benchmark.synthesis, creation_time);

        (iteration_benchmark, tracing_benchmark, synthesis_benchmark)
    } else {
        (None, None, None)
    }
}

fn build_surfel_tables(
    effects: &Vec<EffectSpec>,
    entities: &Vec<Entity>,
    surface: &Surface,
) -> SurfelTableCache {
    let mut surfel_tables = SurfelTableCache::new();

    info!(
        "Surfel table pre-calculation started for {fx_len} effects on {ent_len} entities...",
        fx_len = effects.len(),
        ent_len = entities.len(),
    );

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
                // Do not filter anything if no material name given
                .filter(|(_, e)| is_entity_applicable_for_materials(e, materials))
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
            } => (0..entities.len()).for_each(|idx| {
                surfel_tables.prepare(
                    idx,
                    width,
                    height,
                    surfel_lookup,
                    island_bleed,
                    &entities,
                    surface,
                )
            }),
            _ => (),
        }
    }

    info!("Surfel table pre-calculation complete.");

    surfel_tables
}

fn blend_output_size(blend: &Blend, original_tex_path: Option<&PathBuf>) -> (u32, u32) {
    match (blend.width, blend.height) {
        (Some(w), Some(h)) => (w as u32, h as u32),
        (Some(w), None) => (w as u32, w as u32),
        (None, Some(h)) => (h as u32, h as u32),
        (None, None) => original_tex_path
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
            .expect("Cannot determine surfel table size for layer effect in absence of preferred blend output size. Neither the material nor any blend stop define a loadable texture that could be used to derive a fallback size.")
    }
}

impl fmt::Display for SimulationRunner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Name:               {}\n", self.spec.name)?;
        write!(f, "Description:        {}\n", self.spec.description)?;
        for scene in self.spec.scenes.iter() {
            write!(
                f,
                "Scene:              {}\n",
                scene.file_name().unwrap().to_str().unwrap()
            )?;
        }
        write!(f, "Iterations:         {}\n", self.iterations())?;
        write!(f, "Surfels:            {}\n", self.sim.surfel_count())?;
        write!(f, "Tons per iteration: {}\n", self.sim.emission_count())?;
        write!(f, "Substances:         {:?}", self.unique_substance_names)
    }
}
