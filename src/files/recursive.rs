use std::fs::{create_dir_all, File};
use std::io;
use std::path::PathBuf;

/// Attempts to create or overwrite the file at the given path.
///
/// If it exists and is a file, it will be overwritten.
///
/// If it exists and is a directory or some other non-file entity,
/// an error of kind `io::ErrorKind::InvalidData` is returned.
///
/// If the directory does not exist, the function attempts to create
/// intermediate directories necessary to create it and finally
/// creates and returns the file.
pub fn create_file_recursively<P>(path: P) -> Result<File, io::Error>
where
    P: Into<PathBuf>,
{
    match &path.into() {
        // Path, following symlinks, already exists and is a file, overwrite it
        path if path.is_file() => File::create(&path),
        // Path, following symlinks, already exists and is a directory, fail with specific error message
        path if path.is_dir() => Err(
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Tried to create a file at {}, but a directory already exists at the same path.",
                    path.to_str().unwrap_or("NON-UTF-8")
                )
            )
        ),
        // Some entity exists at path, following symlinks, but it is neither a directory,
        // nor a file. Do not attempt to overwrite and instead fail with generic error message.
        path if path.exists() => Err(
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Tried to create a file at {}, but some entity that is neither a file nor a directory already exists at the same path.",
                    path.to_str().unwrap_or("NON-UTF-8")
                )
            )
        ),
        // Nothing exists at path yet, try to create intermediate directories and the file itself.
        path => {
            if let Some(parent) = path.parent() {
                create_dir_all(parent)?
            }

            File::create(&path)
        }
    }
}
