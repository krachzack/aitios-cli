extern crate aitios_cli;
extern crate failure;
#[macro_use]
extern crate log;

use aitios_cli::app;
use failure::{Error, Fail};
use log::Level::Debug;
use std::process;

fn main() {
    if let Err(err) = app::run() {
        exit_with_error(err);
    }
}

/// Prints error messages and exits with a non-zero exit code.
fn exit_with_error(error: Error) -> ! {
    fail_for_humans(&error);

    // Print additional info if verbose enabled
    if log_enabled!(Debug) {
        fail_for_debugging(error.as_fail());
    }

    process::exit(1)
}

fn fail_for_humans(error: &Error) {
    eprintln!("{}", summarize_error(error));
}

fn summarize_error(error: &Error) -> String {
    let mut causes = error.iter_chain();

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
    debug!("fatal: {:?}", error);
    while let Some(cause) = error.cause() {
        debug!("> cause: {:?}", cause);
        error = cause;
    }
}
