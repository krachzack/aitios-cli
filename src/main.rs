extern crate aitios_geom as geom;
extern crate aitios_asset as asset;
extern crate aitios_sim as sim;
extern crate aitios_surf as surf;
extern crate aitios_scene as scene;
extern crate aitios_tex as tex;
extern crate clap;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_yaml;
extern crate rayon;
#[macro_use]
extern crate log;
extern crate simplelog;

mod spec;
mod runner;

use clap::{ArgMatches, Arg, App};
use spec::SimulationSpec;
use std::fs::File;
use std::path::Path;
use simplelog::{SharedLogger, CombinedLogger, TermLogger, WriteLogger, LevelFilter, Config};

fn main() {
    let matches = App::new("aitios")
        .version("0.1")
        .author("krachzack <hello@phstadler.com>")
        .about("Procedural weathering simulation on the command line with aitios")
        .arg(
            Arg::with_name("SIMULATION_SPEC_FILE")
                .help("Sets the path to the simulation config YAML file")
                .required(true)
                .validator(validate_simulation_spec)
                .index(1)
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("Activates verbose output")
        )
        .arg(
            Arg::with_name("log")
                .short("l")
                .long("log")
                .help("Specifies a file in which to log output")
        )
        .get_matches();

    configure_logging(&matches);

    let spec_file_path = matches.value_of("SIMULATION_SPEC_FILE")
        .expect("No simulation spec file provided");

    info!("Loading simulation described at \"{}\" and preparing data…", spec_file_path);

    let mut runner = runner::load(spec_file_path)
        .unwrap();

    info!("Simulation is ready.");

    // Log the description line-wise
    for line in format!("{}", runner).lines() {
        info!("{}", line);
    }

    info!("Running…");
    runner.run();

    info!("Finished simulation, done.")
}

fn validate_simulation_spec(simulation_spec_file: String) -> Result<(), String> {
    if Path::new(&simulation_spec_file).is_file() {
        let mut file = match File::open(simulation_spec_file) {
            Ok(file) => file,
            Err(err) => return Err(format!("Simulation spec could not be opened: {}", err))
        };

        // TODO more validation and sanity checks
        let spec : Result<SimulationSpec, _> = serde_yaml::from_reader(&mut file);
        match spec {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Simulation spec could not be parsed: {}", err))
        }
    } else {
        Err(format!("Spec file was specified but did not exist at: {}", simulation_spec_file))
    }
}

fn configure_logging(arg_matches: &ArgMatches) {
    let level_filter = if arg_matches.is_present("verbose") {
        LevelFilter::Debug
    } else {
        LevelFilter::Warn
    };

    let mut loggers : Vec<Box<SharedLogger>> = vec![
        TermLogger::new(level_filter, Config::default()).unwrap()
    ];

    if let Some(log_file_path) = arg_matches.value_of("log") {
        let log_file = File::create(log_file_path)
            .expect("Failed to create logging file");

        loggers.push(
            WriteLogger::new(level_filter, Config::default(), log_file)
        );
    }

    CombinedLogger::init(loggers)
        .expect("Failed to set up combined logger");
}
