extern crate aitios_geom as geom;
extern crate aitios_asset as asset;
extern crate aitios_sim as sim;
extern crate aitios_surf as surf;
extern crate aitios_scene as scene;
extern crate aitios_tex as tex;
extern crate clap;
extern crate chrono;
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
use chrono::prelude::*;
use spec::SimulationSpec;
use std::fs::File;
use std::path::Path;
use std::io;
use std::collections::HashSet;
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
                .multiple(true)
                .takes_value(true)
                .min_values(0)
                .max_values(64)
                .value_name("LOG_FILE")
                .help("Specifies a file in which to log simulation progress")
        )
        .get_matches();

    match configure_logging(&matches) {
        Err(err) => {
            println!("Failed to set up logging: {}", err);
            return;
        }
        _ => ()
    };

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

fn configure_logging(arg_matches: &ArgMatches) -> Result<(), io::Error> {
    // Nothing => warn, -v => Info, -vv => Debug
    let term_level_filter = match arg_matches.occurrences_of("verbose") {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug
    };

    let mut loggers : Vec<Box<SharedLogger>> = vec![
        TermLogger::new(term_level_filter, Config::default()).unwrap()
    ];

    let log_files = arg_matches.values_of("log");
    let fallback_log_filename = &synthesize_datetime_log_filename();

    if let Some(log_files) = log_files {
        // Fall back to synthesized filename with date if option was not provided with a value,
        // e.g. aitios-cli sim.yml -l instead of
        //      aitios-cli sim.yml -l LOGFILE.log
        // and make extra sure the log file names are unique before creating them
        let mut log_files : HashSet<_> = log_files.collect();
        if log_files.is_empty() {
            log_files.insert(fallback_log_filename);
        }

        // Then try to create all files and push a logger
        for file in log_files.into_iter() {
            let file = File::create(file)?;
            loggers.push(
                WriteLogger::new(LevelFilter::Debug, Config::default(), file)
            );
        }
    }

    CombinedLogger::init(loggers)
        .expect("Failed to set up combined logger");

    Ok(())
}

/// Synthesize a default filename if -l or --log is passed without an actual filename.
fn synthesize_datetime_log_filename() -> String {
    // Get RFC3339 formatted datetime with timezone and make it filename safe
    // by replacing colons with underscores, e.g.
    // "2018-01-26T18:30:09.453+00:00" => ""2018-01-26T18_30_09.453+00_00"
    let datetime = Local::now()
        .to_rfc3339()
        .replace(":", "_");

    format!(
        "aitios-log-{date}.log",
        date = datetime
    )
}
