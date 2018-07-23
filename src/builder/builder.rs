use builder::{append, instantiate, Error, ResolveErrorKind};
use chrono::*;
use files::Resolver;
use runner::SimulationRunner;
use serde_yaml;
use spec::SimulationSpec;
use std::default::Default;
use std::env::current_dir;
use std::fs::File;
use std::path::Path;

pub struct SimulationBuilder {
    spec: SimulationSpec,
    /// Precedence:
    /// 1. Absolute paths that do also exist,
    /// 2. Current working directory,
    /// 3. Relative to directory that contains simulation spec fragment,
    ///    in the order they were added.
    resolv: Resolver,
    creation_time: DateTime<Local>,
}

/// Builds simulations from specifications or specification fragments stored in files
/// or in memory.
impl SimulationBuilder {
    /// Initializes a builder with a default spec.
    /// File resolver is initialized to read files from
    /// absolute directories and the current working directory.
    pub fn new() -> Self {
        SimulationBuilder {
            spec: Default::default(),
            resolv: local_resolver(),
            creation_time: Local::now(),
        }
    }

    /// Add an additional base for lookup of files for reading, e.g. texture samples
    /// and simulation geometry scenes.
    ///
    /// Note that this does not affect previous invocations of the builder but only
    /// following ones.
    ///
    /// The precedence for reference is:
    /// 1. Absolute paths that do also exist,
    /// 2. relative to current working directory,
    /// 3. relative to directories added with this function,
    /// 4. relative to directory that contains current simulation spec fragment, if adding with a path.
    #[allow(unused)]
    pub fn add_base_path<P>(mut self, base: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        self.resolv
            .add_base(base)
            .map_err(|e| Error::resolve(e, ResolveErrorKind::BasePath))?;

        Ok(self)
    }

    /// Derives a new resolver from the builder-global resolver
    /// that also resolves relative to the parent of the given
    /// fragment path.
    ///
    /// The builder-global resolver is left unchanged. This function
    /// can be used to temporarily resolve from one additional path
    /// using the returned resolver.
    fn resolver_for(&self, spec_path: &Path) -> Result<Resolver, Error> {
        let mut resolver = self.resolv.clone();

        // Add parent of spec fragment as possible base path
        if let Some(spec_parent) = spec_path.parent() {
            if !spec_parent.as_os_str().is_empty() {
                resolver
                    .add_base(spec_parent)
                    .map_err(|e| Error::resolve(e, ResolveErrorKind::Simulation))?;
            }
        }

        Ok(resolver)
    }

    /// Appends a simulation spec YAML file to the mix.
    /// If the file defines already defined properties, they will get merged with previous ones, e.g.
    /// new ton sources will be appended to the existing ones.
    pub fn append_spec_fragment_file<P>(self, simulation_spec_file: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let simulation_spec_file = simulation_spec_file.as_ref();

        // Allow relative files relative to parent of spec.
        let spec_path = self
            .resolver_for(&simulation_spec_file)?
            .resolve(simulation_spec_file)
            .map_err(|e| Error::resolve(e, ResolveErrorKind::Simulation))?;

        let spec = serde_yaml::from_reader(
            // The resolved path should be always openable,
            // except with permission errors
            File::open(&spec_path)?,
        )?;

        self.append_spec_fragment(&spec)
    }

    pub fn append_spec_fragment_str(self, spec: &str) -> Result<Self, Error> {
        let spec = serde_yaml::from_str(spec)?;
        self.append_spec_fragment(&spec)
    }

    pub fn append_spec_fragment(mut self, spec: &SimulationSpec) -> Result<Self, Error> {
        self.spec = append(self.spec, spec);
        Ok(self)
    }

    /// Gets the current state of the underlying spec being mutated.
    pub fn spec(&self) -> &SimulationSpec {
        &self.spec
    }

    /// Time of instantiation of this builder.
    pub fn creation_time(&self) -> DateTime<Local> {
        self.creation_time
    }

    pub fn build(self) -> Result<SimulationRunner, Error> {
        instantiate(self.spec, &self.resolv, self.creation_time)
    }
}

/// Resolver that resolves absolute files and files relative
/// to local directory. Panics if working directory cannot be
/// canonicalized.
fn local_resolver() -> Resolver {
    let mut resolv = Resolver::new();

    // Add current working directory as base path, and panic
    // if something fails. This can really only fail if the cwd
    // is non-existent.
    resolv
        .add_base(current_dir().expect("Could not get current working directory."))
        .expect("Could not resolve current working directory.");

    resolv
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn append_str() {
        let builder = SimulationBuilder::new()
            .append_spec_fragment_str("name: Funny Test Simulation")
            .unwrap();

        assert_eq!("Funny Test Simulation", &builder.spec().name)
    }
}
