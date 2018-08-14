use asset::err::AssetError;
use files::ResolveError;
use serde_yaml::Error as SerdeYamlError;
use std::fmt;
use std::io;

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "Simulation spec failed to parse.")]
    Parse(#[cause] SerdeYamlError),
    #[fail(display = "{} could not be resolved.", kind)]
    Resolve {
        #[cause]
        cause: ResolveError,
        kind: ResolveErrorKind,
    },
    #[fail(display = "I/O error occurred during simulation loading.")]
    IO(#[cause] io::Error),
    #[fail(display = "Failed to load 3D assets for the simulation.")]
    Asset(#[cause] AssetError),
    #[fail(
        display = "Simulation spec did not specify a material to surfel specification mapping, surface properties unspecified."
    )]
    SurfelSpecsMissing,
    #[fail(
        display = "Simulation spec does not specify any effects, no way to obtain results of simulation."
    )]
    EffectsMissing,
    #[fail(
        display = "Simulation spec does not define any particle sources, no particle emission possible."
    )]
    SourcesMissing,
    #[fail(
        display = "No surfel or ton source specs mention any substance names, no substance transport possible."
    )]
    SubstancesMissing,
    #[fail(display = "Surfel distance has been set to {:?}", _0)]
    InvalidSurfelDistance(Option<f32>),
}

impl Error {
    pub fn resolve(cause: ResolveError, kind: ResolveErrorKind) -> Self {
        Error::Resolve { cause, kind }
    }
}

#[derive(Debug)]
pub enum ResolveErrorKind {
    #[allow(unused)]
    BasePath,
    Simulation,
    TonSourceSpec,
    TonSourceMesh,
    SurfelSpec,
    Scene,
    Layer,
    Benchmark,
}

impl fmt::Display for ResolveErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                &ResolveErrorKind::BasePath => "Custom base path",
                &ResolveErrorKind::Simulation => "Simulation specification",
                &ResolveErrorKind::TonSourceSpec => "Gammaton source specification",
                &ResolveErrorKind::TonSourceMesh => "Gammaton source emission mesh",
                &ResolveErrorKind::SurfelSpec => "Surfel specification",
                &ResolveErrorKind::Scene => "Scene to simulate",
                &ResolveErrorKind::Layer => "Texture sample referenced by layer effect",
                &ResolveErrorKind::Benchmark => "Benchmarking CSV",
            }
        )
    }
}

impl From<SerdeYamlError> for Error {
    fn from(error: SerdeYamlError) -> Self {
        Error::Parse(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IO(error)
    }
}

impl From<AssetError> for Error {
    fn from(error: AssetError) -> Self {
        Error::Asset(error)
    }
}
