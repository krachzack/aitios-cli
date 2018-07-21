extern crate aitios_asset as asset;
extern crate aitios_geom as geom;
extern crate aitios_scene as scene;
extern crate aitios_sim as sim;
extern crate aitios_surf as surf;
extern crate aitios_tex as tex;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate chrono;
#[macro_use]
extern crate serde_derive;
extern crate rayon;
extern crate serde;
extern crate serde_yaml;
#[macro_use]
extern crate log;
extern crate simplelog;

mod bencher;
mod files;
mod run;
mod runner;
mod spec;

use failure::{Error, Fail};
use log::Level::Debug;
use std::process;

fn main() {
    if let Err(err) = run::run() {
        exit_with_error(err);
    }
}

/// Prints error messages and exits with a non-zero exit code.
fn exit_with_error(error: Error) -> ! {
    if log_enabled!(Debug) {
        fail_for_debugging(error.cause());
    } else {
        fail_for_humans(error);
    }
    process::exit(1)
}

fn fail_for_humans(error: Error) {
    eprintln!("{}", summarize_error(error));
}

fn summarize_error(error: Error) -> String {
    let mut causes = error.causes();

    // Error struct guarantees at least one top-level cause.
    let top_level_cause = causes.next().unwrap();
    let summary = format!("fatal: {}", top_level_cause);

    match top_level_cause.cause() {
        // No second cause, done.
        None => summary,
        // If second cause is root cause, don't bother
        // enumerating the cause levels.
        Some(cause) if cause.cause().is_none() => format!("{}\ncause: {}", summary, cause),
        // But do so if more than one cause level.
        Some(_) => (1..).zip(causes).fold(summary, |acc, (idx, cause)| {
            format!(
                "{acc}\ncause {level}: {msg}",
                acc = acc,
                level = idx,
                msg = cause
            )
        }),
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
