//! Where the user's own files are kept.
//!
//! A person's run and a program's run want different answers. The machine's answer is
//! [`dirs::data_local_dir`] — `~/.local/share` on a machine that follows the XDG layout, the
//! local application data directory on Windows — which is right for a run a person makes and
//! wrong for one made by something that must keep its files to itself.
//!
//! Such a run has an answer of its own: [`DATA_HOME_ENV_VAR`], read the way a shell reads
//! `XDG_DATA_HOME`, so what it names is the directory Rorolala's own files sit under rather than
//! those files. A sandbox that named only the XDG variable would be a sandbox on Unix alone:
//! Windows has no `XDG_DATA_HOME` to read, and the run would keep the machine's files in place of
//! its own.

use std::ffi::OsString;
use std::path::PathBuf;

use rorolala_utils_constants::DATA_HOME_ENV_VAR;

/// The directory a run keeps the user's own files under.
///
/// It is [`DATA_HOME_ENV_VAR`] where that names one, and the machine's answer otherwise. What
/// comes back is the directory [`USER_DATA_DIR`](rorolala_utils_constants::USER_DATA_DIR) sits in,
/// not the directory itself, so what a caller does with either answer is the same.
///
/// A variable that is set but empty names nothing, and is answered as if it were not set: an
/// empty path names no directory, and taking it would put the files beside the run instead.
#[must_use]
pub fn data_home() -> Option<PathBuf> {
    home_from(std::env::var_os(DATA_HOME_ENV_VAR))
}

/// The directory a run keeps the user's own files under, given what the environment names.
///
/// Split from [`data_home`] so that what a name means can be checked without setting a variable:
/// the environment of a process is shared, and a test that set one would be a test another test
/// runs beside.
fn home_from(named: Option<OsString>) -> Option<PathBuf> {
    match named {
        Some(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ => dirs::data_local_dir(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::home_from;

    #[test]
    fn a_named_directory_is_the_answer() {
        assert_eq!(
            home_from(Some(PathBuf::from("/somewhere/data").into_os_string())),
            Some(PathBuf::from("/somewhere/data"))
        );
    }

    #[test]
    fn an_empty_name_is_the_machines_answer() {
        assert_eq!(
            home_from(Some(PathBuf::new().into_os_string())),
            dirs::data_local_dir()
        );
    }

    #[test]
    fn a_name_that_is_not_set_is_the_machines_answer() {
        assert_eq!(home_from(None), dirs::data_local_dir());
    }
}
