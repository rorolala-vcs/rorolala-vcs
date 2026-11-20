//! The directory a build's user data lives under.
//!
//! The name is compiled in, by `cargo dev export --userdata-name`, and is not read at run time;
//! the base it sits in is the machine's own local data directory, or the one
//! [`ROLA_DATA_HOME`](rorolala_utils_constants::DATA_HOME_ENV_VAR) names for a run that must keep
//! its files to itself. The two questions stay apart on purpose: a run may move its files, and
//! nothing at run time may move the name a build was made for.

use std::path::{Path, PathBuf};

use rorolala_utils_constants::{USER_DATA_DIR, USER_KEYS_DIR, USERDATA_NAME};

use crate::data_home::data_home;

/// The directory a run keeps the user's own files under.
///
/// It is the base [`data_home`] names, with [`USER_DATA_DIR`] and the compiled
/// [`USERDATA_NAME`] under it. `None` is a machine that names no local data directory, and a run
/// that names none either.
#[must_use]
pub fn user_data_dir() -> Option<PathBuf> {
    data_home().map(|home| user_data_dir_in(&home))
}

/// The directory a run keeps the user's own files under, given the base it sits in.
///
/// Split from [`user_data_dir`] so a caller that already knows the base, an integration suite
/// naming a sandbox, composes the same path without setting a process-wide variable.
#[must_use]
pub fn user_data_dir_in(home: &Path) -> PathBuf {
    named_in(home, USERDATA_NAME)
}

/// The directory keys are kept in, under the user data directory.
///
/// This is the scope a generated pair is installed into, and the one a member is looked for in
/// when the user scope is turned on. `None` is the same absence [`user_data_dir`] answers.
#[must_use]
pub fn user_keys_dir() -> Option<PathBuf> {
    Some(user_data_dir()?.join(USER_KEYS_DIR))
}

/// A user data directory for `name`, under `home`.
///
/// The name is a directory segment and nothing else: a caller hands in a value a build command
/// has already checked, and the tests hand in names of their own.
fn named_in(home: &Path, name: &str) -> PathBuf {
    home.join(USER_DATA_DIR).join(name)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rorolala_utils_constants::{USER_DATA_DIR, USERDATA_NAME};

    use super::{named_in, user_data_dir_in};

    #[test]
    fn the_directory_is_the_base_with_the_name_under_it() {
        let dir = user_data_dir_in(Path::new("/somewhere/data"));

        assert_eq!(
            dir,
            Path::new("/somewhere/data")
                .join(USER_DATA_DIR)
                .join(USERDATA_NAME)
        );
        assert!(dir.ends_with(Path::new(USER_DATA_DIR).join(USERDATA_NAME)));
    }

    #[test]
    fn another_name_is_the_same_shape_with_that_name() {
        assert_eq!(
            named_in(Path::new("/somewhere/data"), "release"),
            Path::new("/somewhere/data/rola/release")
        );
    }

    #[test]
    fn the_compiled_name_is_one_directory_segment() {
        assert_ne!(USERDATA_NAME, "");
        assert!(!USERDATA_NAME.contains(['/', '\\']));
        assert_ne!(USERDATA_NAME, ".");
        assert_ne!(USERDATA_NAME, "..");
    }
}
