//! Implements functionality of the command line tool.
//! Might also be useful for other applications that want to
//! include functionality similar to the command line tool.

mod app;
mod run;

pub use self::app::new_app;
pub use self::run::{run, run_with_args};
