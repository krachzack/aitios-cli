extern crate aitios_geom as geom;
extern crate aitios_asset as asset;
extern crate aitios_sim as sim;
extern crate aitios_surf as surf;
extern crate aitios_scene as scene;
extern crate aitios_tex as tex;
#[macro_use] extern crate clap;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;
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
mod files;

use clap::{ArgMatches, Arg, App, Result as ClapResult, ErrorKind as ClapErrorKind};
use chrono::prelude::*;
use rayon::ThreadPoolBuilder;
use runner::SimulationRunner;
use std::fs::File;
use std::collections::HashSet;
use std::default::Default;
use std::process;
use simplelog::{SharedLogger, CombinedLogger, TermLogger, WriteLogger, LevelFilter, Config};
use failure::{Error, Fail, ResultExt, err_msg};
use log::Level::Debug;

fn main() {
    if let Err(err) = run() {
        exit_with_error(err);
    }
}

/// Prints error messages and exits with a non-zero exit code.
fn exit_with_error(error: Error) -> ! {
    if log_enabled!(Debug) {
        fail_for_debugging(error.cause());
    } else {
        fail_for_humans(error.cause());
    }
    
    process::exit(1)
}

fn fail_for_humans(mut error: &Fail) {
    eprintln!("fatal: {}", error);
    // print causes up to depth 5
    let mut cause_depth_range = 1..=5;
    while let Some(cause) = error.cause() {
        match cause_depth_range.next() {
            Some(depth) => {
                eprintln!("cause {}: {}", depth, cause);
                error = cause;
            },
            None => break
        }
    }
}

fn fail_for_debugging(mut error: &Fail) {
    debug!("Printing debug information about the error before exiting.");
    debug!("fatal: {:?}", error);
    while let Some(cause) = error.cause() {
        debug!("> cause: {:?}", cause);
        error = cause;
    }
}

fn run() -> Result<(), Error> {
    let matches = App::new("aitios")
        .version(crate_version!())
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
        .arg(
            Arg::with_name("threads")
                .short("t")
                .long("threads")
                .takes_value(true)
                .value_name("THREAD_COUNT")
                .validator(validate_thread_count)
                .help("Overrides thread pool size from number of virtual processors to the given thread count")
        )
        .get_matches_safe();


    // Before checking if parsing succeeded, set up logging, falling back to a
    // default logging solution, if parsing failed.
    init_logging(&matches)?;

    match matches {
        // Parsing succeeded, unwrap the result and start loading and running simulation.
        Ok(matches) => {
            init_thread_pool(&matches)?;
            let mut runner = init_simulation_runner(&matches)?;

            info!("Running…");
            runner.run();
            info!("Finished simulation, done.");

            Ok(())
        },
        // Parsing either failed or the user just wanted help or version information
        Err(matches_error) => match matches_error.kind {
            // Those are in many cases not really errors but the user just did not want to run
            // anything right now. Exit the application successfully in these cases.
            // If use_stderr is not false, there were probably some subcommands missing and this
            // is in fact a real error that warrants unsuccessful exit.
            ClapErrorKind::HelpDisplayed | ClapErrorKind::VersionDisplayed if !matches_error.use_stderr() => {  
                println!("{}", matches_error.message);
                Ok(())
            },
            // In all other cases, there was some sort of real error,
            // exit unsuccessfully in these cases.
            _ => Err(From::from(matches_error))
        }
    }
}

/// Initializes logging using the given argument matching result.
///
/// If matching failed, tries to set up terminal only logging and
/// returns Ok(()) if successful, otherwise some Err value..
///
/// If matching was successful, tries to apply the logging config
/// and returns Ok(()) if successful, otherwise some Err value.
fn init_logging(matches: &ClapResult<ArgMatches>) -> Result<(), Error> {
    let terminal_only_matches = Default::default();
    let matches = matches.as_ref().unwrap_or(&terminal_only_matches);

    configure_logging(matches)
        // If config was erroneous, try again with default config
        .or_else(|_| configure_logging(&terminal_only_matches))
}

fn init_thread_pool(matches: &ArgMatches) -> Result<(), Error> {
    if let Some(thread_count) = matches.value_of("THREAD_COUNT") {
        let thread_count = usize::from_str_radix(&thread_count, 10)
            .unwrap(); // Can be unwrapped since validator checks this

        ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build_global()
            .context("Thread pool could not be set up with specified thread count.")?
    }
    Ok(())
}

fn init_simulation_runner(matches: &ArgMatches) -> Result<SimulationRunner, Error> {
    let spec_file_path = matches.value_of("SIMULATION_SPEC_FILE")
        .unwrap(); // Can unwrap since is marked as required and parsing would have failed otherwise

    info!("Loading simulation described at \"{}\" and preparing data…", spec_file_path);

    let runner = runner::load(spec_file_path)
        .context(format!("Simulation specification could not be loaded at {}", spec_file_path))?;

    info!("Simulation is ready.");

    // Log the description line-wise
    for line in format!("{}", runner).lines() {
        info!("{}", line);
    }

    Ok(runner)
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
        .map_err(|e| format!(
            "Invalid thread count specified {count}\nCause: {cause}",
            count = thread_count,
            cause = e
        ))
}

fn configure_logging(arg_matches: &ArgMatches) -> Result<(), Error> {
    // Nothing => warn, -v => Info, -vv => Debug
    let term_level_filter = match arg_matches.occurrences_of("verbose") {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug
    };

    let mut loggers : Vec<Box<SharedLogger>> = vec![
        TermLogger::new(term_level_filter, Config::default())
            .ok_or(err_msg("Failed to set up logging to terminal."))?
    ];

    let log_files = arg_matches.values_of("log");
    let fallback_log_filename = &synthesize_datetime_log_filename();

    if let Some(log_files) = log_files {
        // Fall back to synthesized filename with date if option was not provided with a value,
        // e.g. "aitios-cli sim.yml -l" instead of
        //      "aitios-cli sim.yml -l LOGFILE.log"
        // and make extra sure the log file names are unique before creating them
        let mut log_files : HashSet<_> = log_files.collect();
        if log_files.is_empty() {
            log_files.insert(fallback_log_filename);
        }

        // Then try to create all files and push a logger
        for file in log_files.into_iter() {
            let file = File::create(file)
                .context("Failed to create log file.")?;

            loggers.push(
                WriteLogger::new(LevelFilter::Debug, Config::default(), file)
            );
        }
    }

    CombinedLogger::init(loggers)
        .context("Failed to set up combined logger.")?;

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
