use scene::Entity;
use tex::build_surfel_lookup_table;
use geom::Vertex;
use sim::SurfelData;
use std::collections::HashMap;
use spec::SurfelLookup;
use surf;

type Surface = surf::Surface<surf::Surfel<Vertex, SurfelData>>;

pub struct SurfelTableCache {
    surfel_tables: HashMap<Key, Vec<Vec<(f32, usize)>>>,
}

#[derive(Hash, PartialEq, Eq)]
struct Key {
    entity_idx: usize,
    width: usize,
    height: usize,
    count: usize,
    island_bleed: usize
}

impl SurfelTableCache {
    pub fn new() -> Self {
        Self { surfel_tables: HashMap::new() }
    }

    /// Lazily sets up a surfel table with the defined parameters for the entity with
    /// the given index into the given entity vector.
    ///
    /// If such a table has been requested before, no new table is created. There is
    /// no explicit invalidation method if recalculation is desired, so a new cache
    /// should be created in such cases.
    ///
    /// The cached table is a flat vector of `width` times `height` surfel lists.
    /// Such a surfel list is in turn a `Vec<(f32, usize)>` where `f32` is the
    /// distance to the world space position represented by the texel and the usize
    /// is an index into the samples of the given surface.
    ///
    /// # Panics
    /// This function currently panicks for surfel lookup policies different from
    /// `Nearest(usize)`, since this is not yet supported.
    pub fn prepare(&mut self,
        entity_idx: usize,
        width: usize,
        height: usize,
        surfel_lookup: SurfelLookup,
        island_bleed: usize,
        entities: &Vec<Entity>,
        surface: &Surface
    ) {
        let count = match surfel_lookup {
            SurfelLookup::Nearest { count } => count,
            _ => unimplemented!("Only n nearest surfels can be cached for now, not within r")
        };

        let key = Key {
            entity_idx, width, height, count, island_bleed
        };

        self.surfel_tables.entry(key)
            .or_insert_with(|| build_surfel_lookup_table(
                &entities[entity_idx],
                surface,
                count,
                width,
                height,
                island_bleed)
            );
    }

    /// Looks up a surfel association table with the given parameters and panicks if no such
    /// table has been prepared before.
    ///
    /// The cached table to which a reference is returned, is a flat vector of `width` times
    /// `height` surfel lists. Such a surfel list is in turn a `Vec<(f32, usize)>` where
    /// `f32` is the distance to the world space position represented by the texel and the
    /// `usize` is an index into the samples of the given surface.
    ///
    /// # Panics
    /// This function panics if no corresponding surfel table has been prepared in advance.
    /// It currently also panicks for surfel lookup policies different from `Nearest(usize)`,
    /// since this is not yet supported.
    pub fn lookup(&self,
        entity_idx: usize,
        width: usize,
        height: usize,
        surfel_lookup: SurfelLookup,
        island_bleed: usize
    ) -> &Vec<Vec<(f32, usize)>>
    {
        let count = match surfel_lookup {
            SurfelLookup::Nearest { count } => count,
            _ => unimplemented!("Only n nearest surfels can be cached for now, not within r")
        };

        self.surfel_tables.get(&Key {
            entity_idx, width, height, count, island_bleed
        }).unwrap()
    }
}
