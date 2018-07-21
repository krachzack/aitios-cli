#[derive(Fail, Debug)]
pub enum LoadError {
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
}
