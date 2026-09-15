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
