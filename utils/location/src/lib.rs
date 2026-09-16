#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(async_fn_in_trait)]

use std::future::Future;
use std::io::Error;
use std::path::{Path, PathBuf};

mod helper;
pub use helper::*;

/// A trait for locating the file system
///
/// # Examples
///
/// ```
/// use std::path::{Path, PathBuf};
/// use rorolala_utils_location::Locate;
///
/// struct MyLocation {
///     root: PathBuf,
/// }
///
/// impl Locate for MyLocation {
///     fn locate(cwd: &Path) -> Option<Self> {
///         Some(Self { root: cwd.to_path_buf() })
///     }
///
///     fn get_root(&self) -> &Path {
///         &self.root
///     }
/// }
///
/// let location = MyLocation::locate(Path::new("/tmp/space")).unwrap();
/// let path = location.local_path("some/file.txt")?;
/// assert_eq!(path, PathBuf::from("/tmp/space/some/file.txt"));
/// # Ok::<(), std::io::Error>(())
/// ```
pub trait Locate
where
    Self: Sized,
{
    /// Constructs this type from the input path
    ///
    /// Returns `None` if the location cannot be determined from the input path.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new("/tmp/space")).unwrap();
    /// assert_eq!(location.get_root(), Path::new("/tmp/space"));
    /// ```
    fn locate(cwd: &Path) -> Option<Self>;

    /// Gets the root directory
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new("/tmp/space")).unwrap();
    /// assert_eq!(location.get_root(), Path::new("/tmp/space"));
    /// ```
    fn get_root(&self) -> &Path;

    /// Convert a relative path to an absolute path within the space.
    ///
    /// The path is formatted according to the space's path format configuration.
    ///
    /// # Errors
    ///
    /// This function does not currently return an error, but is fallible to
    /// allow future implementations to report failures.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new("/tmp/space")).unwrap();
    /// let path = location.local_path("some/file.txt")?;
    /// assert_eq!(path, PathBuf::from("/tmp/space/some/file.txt"));
    /// # Ok::<(), std::io::Error>(())
    /// ```
    fn local_path(&self, relative_path: impl AsRef<Path>) -> Result<PathBuf, Error> {
        let raw_path = self.get_root().join(relative_path);
        Ok(raw_path)
    }

    /// Convert an absolute path to a relative path within the space, if possible.
    ///
    /// Returns `None` if the absolute path is not under the space directory.
    ///
    /// # Errors
    ///
    /// This function does not currently return an error, but is fallible to
    /// allow future implementations to report failures.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new("/tmp/space")).unwrap();
    /// let relative = location.to_local_path("/tmp/space/some/file.txt")?;
    /// assert_eq!(relative, Some(PathBuf::from("some/file.txt")));
    ///
    /// let outside = location.to_local_path("/other/file.txt")?;
    /// assert_eq!(outside, None);
    /// # Ok::<(), std::io::Error>(())
    /// ```
    fn to_local_path(&self, absolute_path: impl AsRef<Path>) -> Result<Option<PathBuf>, Error> {
        let path = absolute_path.as_ref();
        let current = self.get_root();
        path.strip_prefix(current)
            .map_or(Ok(None), |result| Ok(Some(result.to_path_buf())))
    }

    /// Canonicalize a relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be canonicalized.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// let canonical = location.canonicalize("Cargo.toml").await?;
    /// assert!(canonical.ends_with("Cargo.toml"));
    /// # Ok(())
    /// # }
    /// ```
    fn canonicalize(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<PathBuf, Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::canonicalize(path).await }
    }

    /// Copy a file from one relative path to another within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the copy operation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// let bytes = location.copy("Cargo.toml", "Cargo.toml.bak").await?;
    /// assert!(bytes > 0);
    /// # tokio::fs::remove_file("Cargo.toml.bak").await?;
    /// # Ok(())
    /// # }
    /// ```
    fn copy(
        &self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
    ) -> impl Future<Output = Result<u64, Error>> + Send {
        let from_path = self.get_root().join(from);
        let to_path = self.get_root().join(to);
        async move { tokio::fs::copy(from_path, to_path).await }
    }

    /// Create a directory at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.create_dir("target/doctest-create-dir").await?;
    /// assert!(location.try_exists("target/doctest-create-dir").await?);
    /// # location.remove_dir("target/doctest-create-dir").await?;
    /// # Ok(())
    /// # }
    /// ```
    fn create_dir(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::create_dir(path).await }
    }

    /// Recursively create a directory and all its parents at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target/doctest/nested/dir").await?;
    /// assert!(location.try_exists("target/doctest/nested/dir").await?);
    /// # location.remove_dir_all("target/doctest").await?;
    /// # Ok(())
    /// # }
    /// ```
    fn create_dir_all(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::create_dir_all(path).await }
    }

    /// Create a hard link from `src` to `dst` within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the hard link cannot be created.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.write("target/doctest-hard-link-src", b"hello").await?;
    /// location.hard_link("target/doctest-hard-link-src", "target/doctest-hard-link-dst").await?;
    /// assert!(location.try_exists("target/doctest-hard-link-dst").await?);
    /// # location.remove_file("target/doctest-hard-link-src").await?;
    /// # location.remove_file("target/doctest-hard-link-dst").await?;
    /// # Ok(())
    /// # }
    /// ```
    fn hard_link(
        &self,
        src: impl AsRef<Path>,
        dst: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let src_path = self.get_root().join(src);
        let dst_path = self.get_root().join(dst);
        async move { tokio::fs::hard_link(src_path, dst_path).await }
    }

    /// Get metadata for a file or directory at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be read.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// let metadata = location.metadata("Cargo.toml").await?;
    /// assert!(metadata.is_file());
    /// # Ok(())
    /// # }
    /// ```
    fn metadata(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<std::fs::Metadata, Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::metadata(path).await }
    }

    /// Read the entire contents of a file at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// let bytes = location.read("Cargo.toml").await?;
    /// assert!(!bytes.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    fn read(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<Vec<u8>, Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::read(path).await }
    }

    /// Read the directory entries at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// let mut entries = location.read_dir(".").await?;
    /// assert!(entries.next_entry().await?.is_some());
    /// # Ok(())
    /// # }
    /// ```
    fn read_dir(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<tokio::fs::ReadDir, Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::read_dir(path).await }
    }

    /// Read the target of a symbolic link at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the link cannot be read.
    ///
    /// # Examples
    ///
    /// The link the example reads is made with `symlink`, Unix's spelling of it — Windows
    /// spells the two kinds apart in `symlink_dir` and `symlink_file`, and asks a privilege
    /// of either — so the example is drawn where a process may make one.
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[cfg(unix)]
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.symlink("Cargo.toml", "target/doctest-read-link").await?;
    /// let target = location.read_link("target/doctest-read-link").await?;
    /// assert!(target.ends_with("Cargo.toml"));
    /// # location.remove_file("target/doctest-read-link").await?;
    /// # Ok(())
    /// # }
    /// # #[cfg(not(unix))]
    /// # fn main() {}
    /// ```
    fn read_link(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<PathBuf, Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::read_link(path).await }
    }

    /// Read the entire contents of a file as a string at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not valid UTF-8.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// let contents = location.read_to_string("Cargo.toml").await?;
    /// assert!(contents.contains("package"));
    /// # Ok(())
    /// # }
    /// ```
    fn read_to_string(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<String, Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::read_to_string(path).await }
    }

    /// Remove an empty directory at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be removed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.create_dir("target/doctest-remove-dir").await?;
    /// location.remove_dir("target/doctest-remove-dir").await?;
    /// assert!(!location.try_exists("target/doctest-remove-dir").await?);
    /// # Ok(())
    /// # }
    /// ```
    fn remove_dir(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::remove_dir(path).await }
    }

    /// Remove a directory and all its contents at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be removed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target/doctest-remove-all/nested").await?;
    /// location.remove_dir_all("target/doctest-remove-all").await?;
    /// assert!(!location.try_exists("target/doctest-remove-all").await?);
    /// # Ok(())
    /// # }
    /// ```
    fn remove_dir_all(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::remove_dir_all(path).await }
    }

    /// Remove a file at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be removed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.write("target/doctest-remove-file", b"hello").await?;
    /// location.remove_file("target/doctest-remove-file").await?;
    /// assert!(!location.try_exists("target/doctest-remove-file").await?);
    /// # Ok(())
    /// # }
    /// ```
    fn remove_file(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::remove_file(path).await }
    }

    /// Rename a file or directory from one relative path to another within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the rename operation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.write("target/doctest-rename-src", b"hello").await?;
    /// location.rename("target/doctest-rename-src", "target/doctest-rename-dst").await?;
    /// assert!(!location.try_exists("target/doctest-rename-src").await?);
    /// assert!(location.try_exists("target/doctest-rename-dst").await?);
    /// # location.remove_file("target/doctest-rename-dst").await?;
    /// # Ok(())
    /// # }
    /// ```
    fn rename(
        &self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let from_path = self.get_root().join(from);
        let to_path = self.get_root().join(to);
        async move { tokio::fs::rename(from_path, to_path).await }
    }

    /// Set permissions for a file or directory at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the permissions cannot be set.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// let mut perms = location.metadata("Cargo.toml").await?.permissions();
    /// let original = perms.clone();
    /// location.set_permissions("Cargo.toml", perms).await?;
    /// location.set_permissions("Cargo.toml", original).await?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_permissions(
        &self,
        relative_path: impl AsRef<Path>,
        perm: std::fs::Permissions,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::set_permissions(path, perm).await }
    }

    /// Create a symbolic link from `src` to `dst` within the space (Unix only).
    ///
    /// # Errors
    ///
    /// Returns an error if the symbolic link cannot be created.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.remove_file("target/doctest-symlink").await.ok();
    /// location.symlink("Cargo.toml", "target/doctest-symlink").await?;
    /// assert!(location.symlink_metadata("target/doctest-symlink").await?.is_symlink());
    /// # location.remove_file("target/doctest-symlink").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(unix)]
    fn symlink(
        &self,
        src: impl AsRef<Path>,
        dst: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let src_path = self.get_root().join(src);
        let dst_path = self.get_root().join(dst);
        async move { tokio::fs::symlink(src_path, dst_path).await }
    }

    /// Create a directory symbolic link from `src` to `dst` within the space (Windows only).
    ///
    /// # Errors
    ///
    /// Returns an error if the symbolic link cannot be created.
    ///
    /// # Examples
    ///
    /// Making a link needs a privilege Windows does not give a process by default, so the
    /// example is checked rather than run.
    ///
    /// ```no_run
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.create_dir("target/doctest-symlink-dir-target").await?;
    /// location.symlink_dir("target/doctest-symlink-dir-target", "target/doctest-symlink-dir").await?;
    /// assert!(location.try_exists("target/doctest-symlink-dir").await?);
    /// # location.remove_dir("target/doctest-symlink-dir").await?;
    /// # location.remove_dir("target/doctest-symlink-dir-target").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(windows)]
    fn symlink_dir(
        &self,
        src: impl AsRef<Path>,
        dst: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let src_path = self.get_root().join(src);
        let dst_path = self.get_root().join(dst);
        async move { tokio::fs::symlink_dir(src_path, dst_path).await }
    }

    /// Create a file symbolic link from `src` to `dst` within the space (Windows only).
    ///
    /// # Errors
    ///
    /// Returns an error if the symbolic link cannot be created.
    ///
    /// # Examples
    ///
    /// Making a link needs a privilege Windows does not give a process by default, so the
    /// example is checked rather than run.
    ///
    /// ```no_run
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.symlink_file("Cargo.toml", "target/doctest-symlink-file").await?;
    /// assert!(location.try_exists("target/doctest-symlink-file").await?);
    /// # location.remove_file("target/doctest-symlink-file").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(windows)]
    fn symlink_file(
        &self,
        src: impl AsRef<Path>,
        dst: impl AsRef<Path>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let src_path = self.get_root().join(src);
        let dst_path = self.get_root().join(dst);
        async move { tokio::fs::symlink_file(src_path, dst_path).await }
    }

    /// Get metadata for a file or directory without following symbolic links.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be read.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// let metadata = location.symlink_metadata("Cargo.toml").await?;
    /// assert!(metadata.is_file());
    /// # Ok(())
    /// # }
    /// ```
    fn symlink_metadata(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<std::fs::Metadata, Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::symlink_metadata(path).await }
    }

    /// Check if a file or directory exists at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the existence check fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// assert!(location.try_exists("Cargo.toml").await?);
    /// assert!(!location.try_exists("no/such/file").await?);
    /// # Ok(())
    /// # }
    /// ```
    fn try_exists(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<bool, Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::try_exists(path).await }
    }

    /// Write data to a file at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// location.create_dir_all("target").await?;
    /// location.write("target/doctest-write", b"hello").await?;
    /// assert_eq!(location.read("target/doctest-write").await?, b"hello");
    /// # location.remove_file("target/doctest-write").await?;
    /// # Ok(())
    /// # }
    /// ```
    fn write(
        &self,
        relative_path: impl AsRef<Path>,
        contents: impl AsRef<[u8]>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        let path = self.get_root().join(relative_path);
        let contents = contents.as_ref().to_vec();
        async move { tokio::fs::write(path, contents).await }
    }

    /// Check if a file or directory exists at the given relative path within the space.
    ///
    /// # Errors
    ///
    /// Returns an error if the existence check fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use rorolala_utils_location::Locate;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// struct MyLocation {
    ///     root: PathBuf,
    /// }
    ///
    /// impl Locate for MyLocation {
    ///     fn locate(cwd: &Path) -> Option<Self> {
    ///         Some(Self { root: cwd.to_path_buf() })
    ///     }
    ///
    ///     fn get_root(&self) -> &Path {
    ///         &self.root
    ///     }
    /// }
    ///
    /// let location = MyLocation::locate(Path::new(".")).unwrap();
    /// assert!(location.exists("Cargo.toml").await?);
    /// assert!(!location.exists("no/such/file").await?);
    /// # Ok(())
    /// # }
    /// ```
    fn exists(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<bool, Error>> + Send {
        let path = self.get_root().join(relative_path);
        async move { tokio::fs::try_exists(path).await }
    }
}
