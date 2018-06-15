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
    ///
    /// The base directory is transformed into its canonical form.
    /// If the canonicalized version is already present in the list of
    /// base paths `Ok(())` is returned without duplicating the entry.
    /// If not already present, is it added to the end, i.e. with the
    /// least priority, and `Ok(())` is returned.
    ///
    /// Returns an `Err(std::io::Error)` if any component of the given path
    /// is not a directory or does not exist.
    ///
    /// Note that the current directory is not automatically added.
    /// It can be added as ".":
    ///
    /// ```
    /// # use files::Resolver;
    /// # let mut resolver = Resolver::new();
    /// resolver.add_base(".")
    /// ```
    pub fn add_base<P : Into<PathBuf>>(&mut self, base: P) -> Result<(), Error> {
        let base = base.into().canonicalize()?;

        if !self.bases.contains(&base) {
            self.bases.push(base);
        }

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
        let mut search_path = search_path.as_ref();

        // If search path is already absolute, first try to canonicalize it and
        // returning it without looking for it in base directories.
        //
        // REVIEW is there some potential for accidents where a pseudo-root also is
        // an absolute file?
        if search_path.is_absolute() {
            match search_path.canonicalize() {
                Ok(canonicalized) => return Ok(canonicalized),
                // If canonicalization of the path failed, e.g. because an
                // intermediate directory did not exist or the final file or
                // directory did not exist, we try to reinterpret the path
                // as relative to one of the bases.
                //
                // This allows to use the bases as a sort of "pseudo-root".
                Err(_) => {
                    // Drop the prefix component like / on unix or C:\ on Windows
                    search_path = search_path.strip_prefix(
                        search_path.iter().next().unwrap()
                    ).unwrap();
                }
            }
        }


        // Otherwise, interpret any path as relative, even if it was a non-existing absolute path.
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

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::{File, remove_file, create_dir, remove_dir};

    #[test]
    fn no_bases_relative_nonexistent() {
        let resolver = Resolver::new();
        let nonexistent_file_name = "holodriodl.txt";
        let nonexistent_file = Path::new(nonexistent_file_name);
        let nonexistent_file_buf = PathBuf::from(nonexistent_file_name);

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
        assert!(
            resolver.resolve(&nonexistent_file_buf).is_err(),
            "Did not add any paths so this lookup should fail"
        );
    }

    #[test]
    fn no_bases_pseudo_absolute_nonexistent() {
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

    // If the file can be found in the current cwd, but "." was not
    // added, it should be reported as nonexistent.
    #[test]
    fn no_bases_relative_existent() {
        let resolver = Resolver::new();
        let test_filename = "resolver_test_file2";

        {
            let _tempfile = File::create(test_filename).unwrap();
            let resolved = resolver.resolve(&test_filename);

            assert!(Path::new(test_filename).exists(), "Expected temp file to be present");
            assert!(resolved.is_err(), "Expected lookup to fail because . was not added as a base");
        }

        // Remove the file used for existence check
        remove_file(test_filename).unwrap();
        assert!(!Path::new(test_filename).exists(), "Expected temp file to get deleted");
    }

    // An existent absolute path should be resolved to its canonicalized form.
    #[test]
    fn no_bases_pseudo_absolute_existent() {
        let resolver = Resolver::new();
        let test_filename = "resolver_test_file3";

        {
            let _tempfile = File::create(test_filename).unwrap();
            let test_filename_absolute = Path::new(test_filename).canonicalize().unwrap();
            assert!(test_filename_absolute.is_absolute());
            assert!(Path::new(test_filename).exists(), "Expected temp file to be present");
            assert!(test_filename_absolute.exists(), "Expected temp file to be present");

            let resolved = resolver.resolve(&test_filename);
            assert!(resolved.is_err(), "Expected lookup to fail because . was not added as a base");

            let resolved = resolver.resolve(&test_filename_absolute);
            assert!(resolved.is_ok(), "Expected lookup to succeed because search path was absolute and existent");
        }

        // Remove the file used for existence check
        remove_file(test_filename).unwrap();
        assert!(!Path::new(test_filename).exists(), "Expected temp file to get deleted");
    }

    #[test]
    fn relative_from_cwd() {
        let mut resolver = Resolver::new();
        // Add the current working directory with "."
        resolver.add_base(".").unwrap();

        let test_filename = "resolver_test_file";

        {
            let _tempfile = File::create(test_filename).unwrap();

            let resolved = resolver.resolve(&test_filename);
            assert!(resolved.is_ok());
            let resolved = resolver.resolve(&String::from(test_filename));
            assert!(resolved.is_ok());
            let resolved = resolver.resolve(&Path::new(test_filename));
            assert!(resolved.is_ok());

            let resolved = resolved.unwrap();
            assert!(resolved.is_absolute());
            assert!(resolved.exists());

            // Pseudo-absolute form /resolver_test_file should also work,
            // treating the base paths as "pseudo-roots"
            let resolved = resolver.resolve(&format!("/{}", test_filename));
            assert!(resolved.is_ok());
        }

        // Remove the file used for existence check
        remove_file(test_filename).unwrap();
    }

    #[test]
    fn relative_from_bases_precedence() {
        let outer_temp = "resolver_test_100";
        let directory = "resolver_test_inner";
        let inner_temp = format!("{}/{}", directory, outer_temp);

        create_dir(directory).unwrap();
        {
            let _outer_temp = File::create(outer_temp).unwrap();
            let _inner_temp = File::create(&inner_temp).unwrap();

            {
                let mut inner_first = Resolver::new();
                inner_first.add_base(directory).unwrap();
                inner_first.add_base(".").unwrap();
                let inner_first_resolve = inner_first.resolve(&outer_temp).unwrap();
                assert!(inner_first_resolve.ends_with(&inner_temp));
            }

            {
                let mut outer_first = Resolver::new();
                outer_first.add_base(".").unwrap();
                outer_first.add_base(directory).unwrap();
                let outer_first_resolve = outer_first.resolve(&outer_temp).unwrap();
                assert!(!outer_first_resolve.ends_with(&inner_temp));
            }
        }



        remove_file(inner_temp).unwrap();
        remove_file(outer_temp).unwrap();
        remove_dir(directory).unwrap();
    }
}
