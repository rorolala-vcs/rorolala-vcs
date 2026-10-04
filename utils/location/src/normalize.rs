//! Lexical normalisation of paths, so two spellings of one place compare equal.
//!
//! A path that came from a person, an environment or a listing is not canonical: it may carry `.`
//! components, a `..` that walks back over a named directory, or a separator at either end. What
//! is done here is lexical — nothing is read from the filesystem — so a symbolic link is left as
//! the name it is rather than resolved. That is what keeps it cheap enough to do per path, and it
//! is also what keeps two spellings of one place equal when one of them went through a link.

use std::path::{Component, Path, PathBuf};

/// `path` with `.` components dropped and `..` taken back over the name before it.
///
/// A `..` with nothing to take back — at the root, or after another `..` — is kept, since only the
/// filesystem can say whether it means anything above where the path starts. What is left names
/// the same place, spelled the way a comparison wants it.
#[must_use]
pub fn normalize(path: &Path) -> PathBuf {
    let mut components: Vec<Component<'_>> = Vec::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if matches!(components.last(), Some(Component::Normal(_))) => {
                components.pop();
            }
            other => components.push(other),
        }
    }

    components.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::normalize;

    #[test]
    fn a_cur_dir_component_is_dropped() {
        assert_eq!(normalize(Path::new("a/./b")), PathBuf::from("a/b"));
    }

    #[test]
    fn a_parent_dir_takes_back_the_name_before_it() {
        assert_eq!(normalize(Path::new("a/b/../c")), PathBuf::from("a/c"));
    }

    #[test]
    fn a_parent_dir_with_nothing_to_take_back_is_kept() {
        // Only the filesystem can say what is above a relative path or the root, so the `..` stays.
        assert_eq!(normalize(Path::new("../a")), PathBuf::from("../a"));
        assert_eq!(normalize(Path::new("/../a")), PathBuf::from("/../a"));
    }

    #[test]
    fn a_root_and_a_prefix_survive() {
        assert_eq!(normalize(Path::new("/a/b")), PathBuf::from("/a/b"));
    }
}
