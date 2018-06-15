use std::path::{Path, PathBuf};
use std::io::{Error, ErrorKind};

/// Resolves relative and absolute filenames using a list
/// of base paths that the filenames for lookup can be
/// relative to.
pub struct Resolver {
    bases: Vec<PathBuf>
}

impl Resolver {
    pub fn new() -> Self {
        Self { bases: Vec::new() }
    }

    /// Adds a base directory for later calls to resolve.
    pub fn add_base<P : Into<PathBuf>>(&mut self, base: P) -> Result<(), Error> {
        let base = base.into().canonicalize()?;
        self.bases.push(base);
        Ok(())
    }

    /// Looks up the given search path in a list of base paths and
    /// returns an absolute, canonicalized path.
    ///
    /// If search path is already absolute, checks if it exists.
    /// If it does exist, it is returned in its canonicalized form,
    /// if it does not exist, it is interpreted as relative to one of
    /// the base paths.
    ///
    /// If search path is relative or absolute and non-existent, it is
    /// searched for within each added base path, in the order they were
    /// added. The first existing file or directory is returned.
    ///
    /// If no base path contains the given search path, returns a not found
    /// error.
    ///
    /// # Examples
    /// Finding `/dev/rand` and `/dev/null` can be done by adding a base
    /// path `/dev/`.
    ///
    /// ```
    /// use files::Resolver;
    ///
    /// let resolver = Resolver::new();
    ///
    /// // Resolving rand and null should fail since no base
    /// // paths have been added yet.
    /// assert!(resolver.resolve("rand").is_err(), "No path added but found rand file");
    /// assert!(resolver.resolve("null").is_err(), "No path added but found null file");
    ///
    /// if cfg!(unix) {
    ///     // Most unix-like systems have /dev/rand and /dev/null,
    ///     // let's try finding them.
    ///
    ///     // First using existing absolute paths.
    ///     assert!(resolver.resolve("/dev/rand").is_ok());
    ///     assert!(resolver.resolve("/dev/randidliodl").is_err()); // This on does not exist though and fails
    ///
    ///     // Then, try using base paths
    ///     resolver.add_base("/dev");
    ///     assert!(
    ///         resolver.resolve("rand").is_ok() &&
    ///         resolver.resolve("/rand").is_ok(), // psuedo-root is also allowed (absolute but non-existent)
    ///         "On Unix-like system but no /dev/rand found or available"
    ///     );
    ///     assert!(
    ///         resolver.resolve("null").is_ok(),
    ///         "On Unix-like system but no /dev/null found or available"
    ///     );
    /// }
    /// ```
    pub fn resolve<P : AsRef<Path>>(&self, search_path: &P) -> Result<PathBuf, Error> {
        let search_path = search_path.as_ref();

        if search_path.is_absolute() && search_path.exists() {
            // If an existing, absolute path is given, immediately return it.
            let canonicalized = PathBuf::from(search_path).canonicalize()?;
            Ok(canonicalized)
        } else {
            // Otherwise, interpret as relative, even if it was a non-existing absolute path.
            for mut resolve_attempt in self.bases.iter().cloned() {
                resolve_attempt.push(search_path);
                let resolve_attempt = resolve_attempt.canonicalize()?;

                if resolve_attempt.exists() {
                    return Ok(resolve_attempt);
                }
            }

            Err(
                Error::new(
                    ErrorKind::NotFound,
                    format!(
                        "Search path {:?} could not be found in potential base paths {:?}",
                        search_path,
                        &self.bases
                    )
                )
            )
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn no_bases_relative() {
        let resolver = Resolver::new();
        let nonexistent_file_name = "holodriodl.txt";
        let nonexistent_file = Path::new(nonexistent_file_name);

        assert!(
            !nonexistent_file.exists(),
            "holodriodl.txt was supposed to never be a real file, but you made one and broke the test"
        );

        println!("{:?}", resolver.resolve(&nonexistent_file_name));

        // Resolving &str, &String and &Path should all work
        assert!(
            resolver.resolve(&nonexistent_file_name).is_err(),
            "Did not add any paths so this lookup should fail"
        );
        assert!(
            resolver.resolve(&String::from(nonexistent_file_name)).is_err(),
            "Did not add any paths so this lookup should fail"
        );
        assert!(
            resolver.resolve(&nonexistent_file).is_err(),
            "Did not add any paths so this lookup should fail"
        );
    }

    #[test]
    fn no_bases_pseudo_absolute() {
        let resolver = Resolver::new();
        let nonexistent_file_name = "/holodriodl.txt";
        let nonexistent_file = Path::new(nonexistent_file_name);

        assert!(
            !nonexistent_file.exists(),
            "/holodriodl.txt was supposed to never be a real file, but you made one and broke the test"
        );

        println!("{:?}", resolver.resolve(&nonexistent_file_name));

        // Resolving &str, &String and &Path should all work
        assert!(
            resolver.resolve(&nonexistent_file_name).is_err(),
            "Did not add any paths so this lookup should fail"
        );
        assert!(
            resolver.resolve(&String::from(nonexistent_file_name)).is_err(),
            "Did not add any paths so this lookup should fail"
        );
        assert!(
            resolver.resolve(&nonexistent_file).is_err(),
            "Did not add any paths so this lookup should fail"
        );
    }
}
