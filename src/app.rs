use clap::{App, Arg};

pub fn new_app<'a, 'b>() -> App<'a, 'b> {
    App::new("aitios")
        .version(crate_version!())
        .author("krachzack <hello@phstadler.com>")
        .about("Procedural weathering simulation on the command line with aitios")
        .arg(
            Arg::with_name("SIMULATION_SPEC_FILE")
                .help("Adds a new simulation specification fragment in a YAML file at the given path.")
                .long_help("Adds a new simulation specification fragment in a YAML file at the given path. Multiple specs can be provided and later specs will add to or even override earlier specs, depending on the property. See --spec to provide an inline specification without a file.")
                .required(true)
                .validator(validate_simulation_spec)
                .multiple(true)
                .takes_value(true)
        )
        .arg(
            Arg::with_name("spec")
                .short("s")
                .long("spec")
                .multiple(true)
                .takes_value(true)
                .help("Evaluates the given simulation spec directly")
                .long_help("Evaluates the given simulation specification directly. It must be provided as a string in YAML format.")
                .value_name("INLINE_SIMULATION_SPEC")
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("Activates verbose output.")
        )
        .arg(
            Arg::with_name("log")
                .short("l")
                .long("log")
                .multiple(true)
                .takes_value(true)
                .min_values(0)
                .max_values(64)
                .value_name("LOG_FILE")
                .help("Specifies a file in which to log simulation progress.")
        )
        .arg(
            Arg::with_name("threads")
                .short("t")
                .long("threads")
                .takes_value(true)
                .value_name("THREAD_COUNT")
                .validator(validate_thread_count)
                .help("Overrides thread pool size from number of virtual processors to the given thread count.")
        )
}

fn validate_simulation_spec(simulation_spec_file: String) -> Result<(), String> {
    if simulation_spec_file.is_empty() {
        return Err("Specified simulation spec file path is empty".into());
    }

    Ok(())
}

fn validate_thread_count(thread_count: String) -> Result<(), String> {
    usize::from_str_radix(&thread_count, 10)
        .map(|_| ())
        .map_err(|e| {
            format!(
                "Invalid thread count specified: {count}\nCause: {cause}",
                count = thread_count,
                cause = e
            )
        })
}
