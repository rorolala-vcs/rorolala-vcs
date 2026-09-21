use std::path::{Path, PathBuf};

/// A helper for locating the root directory
pub trait LocateHelper {
    /// Starting from the current path, checks each parent level
    /// until it finds the first path that satisfies the `matcher` condition and returns it.
    /// If no path satisfies the condition all the way up to the root directory, returns `None`.
    fn locate<F>(&self, matcher: F) -> Option<PathBuf>
    where
        F: Fn(&Path) -> bool;
}

impl<T> LocateHelper for T
where
    T: AsRef<Path>,
{
    fn locate<F>(&self, matcher: F) -> Option<PathBuf>
    where
        F: Fn(&Path) -> bool,
    {
        let mut current = Some(self.as_ref());
        while let Some(path) = current {
            if matcher(path) {
                return Some(path.to_path_buf());
            }
            current = path.parent();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::LocateHelper;
    use std::cell::Cell;
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    #[test]
    fn the_start_path_itself_is_checked_before_any_parent() {
        let found = Path::new("/a/b/c").locate(|path| path == Path::new("/a/b/c"));
        assert_eq!(found, Some(PathBuf::from("/a/b/c")));
    }

    #[test]
    fn the_first_parent_the_matcher_accepts_is_the_one_returned() {
        let found = Path::new("/a/b/c/d").locate(|path| path.file_name() == Some(OsStr::new("b")));
        assert_eq!(found, Some(PathBuf::from("/a/b")));
    }

    #[test]
    fn a_matcher_that_never_accepts_walks_to_the_root_and_gives_up() {
        let seen = Cell::new(0);
        let found = Path::new("/a/b/c").locate(|_| {
            seen.set(seen.get() + 1);
            false
        });
        assert_eq!(found, None);
        // `/a/b/c`, `/a/b`, `/a`, `/` and then the root's parent, `None`.
        assert_eq!(seen.get(), 4);
    }
}
