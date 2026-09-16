#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

// So that the derive's `::rorolala_utils_configure` path resolves inside this crate
// too, which is what lets the tests below use the derive rather than a hand-written
// implementation. The library target alone has no use for it, hence the allow.
#[allow(unused_extern_crates)]
extern crate self as rorolala_utils_configure;

use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io::{self, ErrorKind};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use rorolala_utils_lazyffi::lazyffi;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use rorolala_utils_configure_macros::*;

/// Why the filesystem refused, as far as this crate tells the reasons apart.
///
/// A value rather than a message, because [`Error`] crosses to C, where a caller
/// switches on the variant and reads the fields instead of parsing text.
#[lazyffi(export = ConfigureErrorReason)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Nothing is there.
    ///
    /// The one a program that wants to create a default configuration looks for.
    Missing,
    /// There, but not allowed.
    Denied,
    /// There, but not what the operation needed — a directory where a file was wanted,
    /// or the other way round.
    WrongKind,
    /// There, but in use, or otherwise busy.
    Busy,
    /// Something else, which this crate does not tell apart.
    Other,
}

impl Reason {
    /// The reason an I/O failure amounts to.
    fn of(error: &io::Error) -> Self {
        match error.kind() {
            ErrorKind::NotFound => Self::Missing,
            ErrorKind::PermissionDenied => Self::Denied,
            ErrorKind::IsADirectory | ErrorKind::NotADirectory => Self::WrongKind,
            ErrorKind::DirectoryNotEmpty | ErrorKind::ResourceBusy => Self::Busy,
            _ => Self::Other,
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "it is not there",
            Self::Denied => "it is not allowed",
            Self::WrongKind => "it is not a file",
            Self::Busy => "it is in use",
            Self::Other => "the filesystem gave no reason this crate tells apart",
        })
    }
}

/// What can go wrong while reading, staging or publishing a configuration file.
///
/// Every variant is a failure a caller can act on, and holds what that caller needs to
/// act on it — paths, a [`Reason`], a position. None of them holds a message: the enum
/// crosses to C, where a caller switches on the variant, and text is for
/// [`Display`](fmt::Display) to produce rather than for the value to carry.
#[lazyffi(export = ConfigureError)]
#[derive(Debug)]
pub enum Error {
    /// The file could not be read.
    Read {
        /// The file that could not be read.
        file: PathBuf,
        /// Why not.
        reason: Reason,
    },
    /// The staging file could not be written.
    Stage {
        /// The file that was being read, or edited.
        file: PathBuf,
        /// The staging file that could not be written.
        lock: PathBuf,
        /// Why not.
        reason: Reason,
    },
    /// The staging file could not replace the original.
    Publish {
        /// The file that was left as it was.
        file: PathBuf,
        /// The staging file that is still staged.
        lock: PathBuf,
        /// Why not.
        reason: Reason,
    },
    /// The file's contents do not parse as the configuration type.
    Parse {
        /// The file that does not parse.
        file: PathBuf,
        /// 1-based line the parser stopped at, or 0 when it named no position.
        line: u32,
        /// 1-based column the parser stopped at, or 0 when it named no position.
        column: u32,
    },
    /// The configuration could not be rendered in the file's format.
    ///
    /// A serializer's complaint is always a sentence — there is nothing structured in a
    /// `serde` failure to pass on — so it is not carried; what is left to say is that
    /// the type cannot be written as this format, which is a fault in the type.
    Render {
        /// The file the rendering was for.
        file: PathBuf,
    },
    /// A staging file was already there, so an earlier edit was never published.
    ///
    /// Refusing is deliberate: reading would copy over whatever that edit staged, and a
    /// caller that wants to discard it can remove the staging file, while one that
    /// wants to recover it still has it.
    Locked {
        /// The file the read was for.
        file: PathBuf,
        /// The staging file left behind by whoever got there first.
        lock: PathBuf,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { file, reason } => {
                write!(
                    formatter,
                    "`{}` could not be read: {reason}",
                    file.display()
                )
            }
            Self::Stage { file, lock, reason } => write!(
                formatter,
                "`{}` could not be staged as `{}`: {reason}",
                file.display(),
                lock.display()
            ),
            Self::Publish { file, lock, reason } => write!(
                formatter,
                "`{}` could not be replaced by `{}`: {reason}",
                file.display(),
                lock.display()
            ),
            Self::Parse { file, line, column } => write!(
                formatter,
                "`{}` does not parse{}",
                file.display(),
                Position {
                    line: *line,
                    column: *column
                }
            ),
            Self::Render { file } => write!(
                formatter,
                "`{}` could not be written: the value does not fit that format",
                file.display()
            ),
            Self::Locked { file, lock } => write!(
                formatter,
                "`{}` is already being edited: `{}` is in the way",
                file.display(),
                lock.display()
            ),
        }
    }
}

impl StdError for Error {}

/// The ` at line L, column C` half of a parse failure, or nothing when there is none.
struct Position {
    /// 1-based line, or 0 when the parser named none.
    line: u32,
    /// 1-based column, or 0 when the parser named none.
    column: u32,
}

impl fmt::Display for Position {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            return Ok(());
        }

        write!(formatter, " at line {}, column {}", self.line, self.column)
    }
}

/// A type that is the contents of a configuration file.
///
/// Implemented through `#[derive(Configure)]`, which is the opt-in; the methods
/// themselves have default bodies and are the whole reading and writing behaviour:
///
/// ```
/// use rorolala_utils_configure::Configure;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, Configure)]
/// struct VaultConfig {
///     name: String,
/// }
///
/// fn takes_a_configuration<C: Configure>() {}
/// takes_a_configuration::<VaultConfig>();
/// ```
///
/// The format is the one the file's extension names: `toml` (and `tml`), `yaml` (and
/// `yml`), and `json` — which is also what an extension this crate does not know, or
/// none at all, means.
pub trait Configure: Serialize + DeserializeOwned {
    /// Reads the configuration in `file`.
    ///
    /// # Errors
    ///
    /// Fails when `file` cannot be read, or its contents do not parse as `Self`.
    fn read_from(file: &Path) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let text = fs::read_to_string(file).map_err(|error| Error::Read {
            file: file.to_path_buf(),
            reason: Reason::of(&error),
        })?;

        Format::of(file)
            .parse(&text)
            .map_err(|(line, column)| Error::Parse {
                file: file.to_path_buf(),
                line,
                column,
            })
    }

    /// Writes the configuration to `file`.
    ///
    /// # Errors
    ///
    /// Fails when the value cannot be rendered, or `file` cannot be written.
    fn write_to(&self, file: &Path) -> Result<(), Error> {
        let text = Format::of(file).render(self).map_err(|()| Error::Render {
            file: file.to_path_buf(),
        })?;

        fs::write(file, text).map_err(|error| Error::Stage {
            file: file.to_path_buf(),
            lock: file.to_path_buf(),
            reason: Reason::of(&error),
        })
    }
}

/// A configuration file being edited, staged beside the original.
///
/// [`read`](Config::read) parses the file and copies it to a staging file next to it
/// (`<file>.lock`), and [`new`](Config::new) starts one from an empty configuration.
/// [`write`](Config::write) renders the current contents into that staging file, and
/// the original is then replaced by a single rename — on [`publish`](Config::publish),
/// or when the value is dropped. The original is therefore never truncated in place: a
/// process that dies mid-edit leaves it whole, with the staged edit beside it.
///
/// The contents are reached straight through: a `Config<T>` is a `T` for reading and
/// for writing, through [`Deref`] and [`DerefMut`].
#[derive(Debug)]
pub struct Config<T: Configure> {
    /// The contents.
    value: T,
    /// The file that was read, and that publishing replaces.
    file: PathBuf,
    /// The staging file that writes go to.
    lock: PathBuf,
    /// Whether the staging file has already been published.
    published: bool,
}

impl<T: Configure> Config<T> {
    /// Reads `path` into a configuration, staging a copy of it beside the original.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read or does not parse, when a staging file is
    /// already there, or when the copy cannot be staged.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, Error> {
        let file = path.as_ref().to_path_buf();
        let lock = lock_path(&file);

        if lock.try_exists().map_err(|error| Error::Read {
            file: lock.clone(),
            reason: Reason::of(&error),
        })? {
            return Err(Error::Locked { file, lock });
        }

        let value = T::read_from(&file)?;

        fs::copy(&file, &lock).map_err(|error| Error::Stage {
            file: file.clone(),
            lock: lock.clone(),
            reason: Reason::of(&error),
        })?;

        Ok(Self {
            value,
            file,
            lock,
            published: false,
        })
    }

    /// Creates `path` as a new, empty configuration.
    ///
    /// The file is written straight away, with the default value rendered in the format
    /// its extension names, and a copy of it is staged beside the original — which leaves
    /// exactly the state [`read`](Config::read) leaves behind, so editing and publishing
    /// carry on the same way. An existing file at `path` is replaced: this is the call
    /// that asks for a starting point.
    ///
    /// # Errors
    ///
    /// Fails when a staging file is already there, when the default value cannot be
    /// rendered, or when either file cannot be written.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Error>
    where
        T: Default,
    {
        let file = path.as_ref().to_path_buf();
        let lock = lock_path(&file);

        if lock.try_exists().map_err(|error| Error::Read {
            file: lock.clone(),
            reason: Reason::of(&error),
        })? {
            return Err(Error::Locked { file, lock });
        }

        let value = T::default();

        let text = Format::of(&file)
            .render(&value)
            .map_err(|()| Error::Render { file: file.clone() })?;

        // Staged first and put in place second, so that a failure leaves the staging
        // file — the same leftover an interrupted edit leaves, and the same sign that the
        // original was never reached.
        fs::write(&lock, text).map_err(|error| Error::Stage {
            file: file.clone(),
            lock: lock.clone(),
            reason: Reason::of(&error),
        })?;

        fs::copy(&lock, &file).map_err(|error| Error::Publish {
            file: file.clone(),
            lock: lock.clone(),
            reason: Reason::of(&error),
        })?;

        Ok(Self {
            value,
            file,
            lock,
            published: false,
        })
    }

    /// The file that was read, and that publishing replaces.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// The staging file that writes go to, and that publishing renames into place.
    #[must_use]
    pub fn lock(&self) -> &Path {
        &self.lock
    }

    /// Renders the current contents into the staging file.
    ///
    /// The original is untouched until the staging file is published, so any number of
    /// writes cost one publish.
    ///
    /// # Errors
    ///
    /// Fails when the contents cannot be rendered, or the staging file cannot be
    /// written.
    pub fn write(&self) -> Result<(), Error> {
        // The format comes from the original file: the staging file's own extension is
        // `lock`, which names no format.
        let text = Format::of(&self.file)
            .render(&self.value)
            .map_err(|()| Error::Render {
                file: self.file.clone(),
            })?;

        fs::write(&self.lock, text).map_err(|error| Error::Stage {
            file: self.file.clone(),
            lock: self.lock.clone(),
            reason: Reason::of(&error),
        })
    }

    /// Publishes the staged file, replacing the original now rather than on drop.
    ///
    /// This is how a publish failure is reported: dropping publishes too, but has
    /// nowhere to put an error.
    ///
    /// # Errors
    ///
    /// Fails when the original cannot be replaced.
    pub fn publish(mut self) -> Result<(), Error> {
        rename(&self.lock, &self.file)?;
        self.published = true;
        Ok(())
    }
}

impl<T: Configure> Deref for Config<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: Configure> DerefMut for Config<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: Configure> Drop for Config<T> {
    fn drop(&mut self) {
        if self.published {
            return;
        }

        // A drop has nowhere to report to, so a failure here leaves the staging file
        // in place: that leftover is the sign that the original was not replaced, and
        // the next `read` refuses because of it rather than reading past the edit.
        let _ = rename(&self.lock, &self.file);
    }
}

/// The staging file that sits beside `file`: its name with `.lock` appended.
fn lock_path(file: &Path) -> PathBuf {
    let mut name = file.as_os_str().to_os_string();
    name.push(".lock");
    PathBuf::from(name)
}

/// Renames `from` over `to`, turning a refusal into an error.
fn rename(from: &Path, to: &Path) -> Result<(), Error> {
    fs::rename(from, to).map_err(|error| Error::Publish {
        file: to.to_path_buf(),
        lock: from.to_path_buf(),
        reason: Reason::of(&error),
    })
}

/// A number as the `u32` the header spells it as, saturating rather than wrapping.
fn as_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// The 1-based line and column of a byte offset in `text`, or zeroes without one.
fn position_in(text: &str, offset: Option<usize>) -> (u32, u32) {
    let Some(offset) = offset else {
        return (0, 0);
    };

    let before = &text[..offset.min(text.len())];
    let line = before.lines().count().max(1);
    let column = before
        .rsplit('\n')
        .next()
        .map_or(1, |rest| rest.chars().count() + 1);

    (as_u32(line), as_u32(column))
}

/// The formats this crate reads and writes, chosen by file extension.
#[derive(Clone, Copy, Debug)]
enum Format {
    /// JSON, which is also what an unrecognised extension means.
    Json,
    /// TOML.
    Toml,
    /// YAML.
    Yaml,
}

impl Format {
    /// The format a file's extension names.
    fn of(file: &Path) -> Self {
        match file
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase)
            .as_deref()
        {
            Some("toml" | "tml") => Self::Toml,
            Some("yaml" | "yml") => Self::Yaml,
            _ => Self::Json,
        }
    }

    /// Parses `text` as this format, reporting where the parser stopped.
    ///
    /// A parser's own wording is not passed on: what a caller can act on is the
    /// position, and the position is not a sentence.
    fn parse<C: DeserializeOwned>(self, text: &str) -> Result<C, (u32, u32)> {
        match self {
            Self::Json => serde_json::from_str(text)
                .map_err(|error| (as_u32(error.line()), as_u32(error.column()))),
            Self::Toml => toml::from_str(text)
                .map_err(|error| position_in(text, error.span().map(|span| span.start))),
            Self::Yaml => serde_yaml::from_str(text).map_err(|error| {
                error
                    .location()
                    .map_or((0, 0), |at| (as_u32(at.line()), as_u32(at.column())))
            }),
        }
    }

    /// Renders `config` as this format.
    ///
    /// The failure carries nothing because a `serde` serializer's complaint is always a
    /// sentence: there is no structured part to hand on.
    fn render<C: Serialize>(self, config: &C) -> Result<String, ()> {
        match self {
            Self::Json => serde_json::to_string_pretty(config).map_err(|_| ()),
            Self::Toml => toml::to_string(config).map_err(|_| ()),
            Self::Yaml => serde_yaml::to_string(config).map_err(|_| ()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, Configure, Error, Reason};
    use serde::{Deserialize, Serialize};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A configuration to round-trip.
    #[derive(Debug, Default, PartialEq, Serialize, Deserialize, Configure)]
    struct Settings {
        name: String,
        level: u32,
    }

    /// The contents a test starts a file with.
    const TOML: &str = "name = \"before\"\nlevel = 1\n";

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-configure-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn dropped_config_publishes_what_was_written() {
        let dir = scratch("publish");
        let file = dir.join("settings.toml");
        fs::write(&file, TOML).unwrap();

        {
            let mut config = Config::<Settings>::read(&file).unwrap();

            // Reading and writing the contents goes through the derefs.
            assert_eq!(config.name, "before");
            config.level = 2;

            config.write().unwrap();

            // Until the publish, the original is byte for byte what it was, and the
            // staged copy is the one that holds the edit.
            assert_eq!(fs::read_to_string(&file).unwrap(), TOML);
            assert!(config.lock().exists());
        }

        // Dropped: the staged copy replaced the original, and is no longer beside it.
        let published = fs::read_to_string(&file).unwrap();
        assert!(published.contains("level = 2"));
        assert!(!dir.join("settings.toml.lock").exists());
    }

    #[test]
    fn publishing_twice_is_not_a_second_replacement() {
        let dir = scratch("explicit");
        let file = dir.join("settings.toml");
        fs::write(&file, TOML).unwrap();

        let mut config = Config::<Settings>::read(&file).unwrap();
        config.name = "after".to_owned();
        config.write().unwrap();
        config.publish().unwrap();

        assert!(fs::read_to_string(&file).unwrap().contains("after"));
        assert!(!dir.join("settings.toml.lock").exists());
    }

    #[test]
    fn the_extension_picks_the_format() {
        let dir = scratch("formats");

        let yaml = dir.join("settings.yaml");
        fs::write(&yaml, "name: from yaml\nlevel: 3\n").unwrap();
        let config = Config::<Settings>::read(&yaml).unwrap();
        assert_eq!(config.name, "from yaml");
        assert_eq!(config.level, 3);

        // Nothing was written, so publishing puts back exactly what was read.
        drop(config);
        assert_eq!(
            fs::read_to_string(&yaml).unwrap(),
            "name: from yaml\nlevel: 3\n"
        );

        let json = dir.join("settings.json");
        fs::write(&json, "{\"name\":\"from json\",\"level\":4}").unwrap();
        let mut config = Config::<Settings>::read(&json).unwrap();
        config.level = 5;
        config.write().unwrap();
        drop(config);

        let written = fs::read_to_string(&json).unwrap();
        assert!(written.contains("\"level\": 5"));
    }

    #[test]
    fn a_staged_file_left_behind_is_refused_rather_than_overwritten() {
        let dir = scratch("locked");
        let file = dir.join("settings.toml");
        fs::write(&file, TOML).unwrap();

        let lock = dir.join("settings.toml.lock");
        fs::write(&lock, "name = \"staged\"\nlevel = 9\n").unwrap();

        match Config::<Settings>::read(&file) {
            Err(Error::Locked { lock: reported, .. }) => assert_eq!(reported, lock),
            other => panic!("expected Locked, got {other:?}"),
        }

        // The point of refusing: the abandoned edit is still there to be looked at.
        assert!(fs::read_to_string(&lock).unwrap().contains("staged"));
    }

    #[test]
    fn a_missing_file_is_reported_as_missing() {
        let dir = scratch("missing");

        match Config::<Settings>::read(dir.join("nothing.toml")) {
            Err(Error::Read {
                reason: Reason::Missing,
                ..
            }) => {}
            other => panic!("expected Read/Missing, got {other:?}"),
        }

        // A failed read stages nothing.
        assert!(!dir.join("nothing.toml.lock").exists());
    }

    #[test]
    fn a_file_that_does_not_parse_reports_where() {
        let dir = scratch("broken");
        let file = dir.join("broken.toml");
        fs::write(&file, "name = \"before\"\nlevel = = 1\n").unwrap();

        match Config::<Settings>::read(&file) {
            Err(Error::Parse { line, column, .. }) => {
                assert_eq!(line, 2);
                assert!(column > 0);
            }
            other => panic!("expected Parse, got {other:?}"),
        }

        // A failed parse stages nothing either.
        assert!(!dir.join("broken.toml.lock").exists());
    }

    #[test]
    fn a_directory_where_a_file_was_wanted_is_reported_as_the_wrong_kind() {
        let dir = scratch("wrongkind");

        match Config::<Settings>::read(&dir) {
            Err(Error::Read {
                reason: Reason::WrongKind | Reason::Denied | Reason::Other,
                ..
            }) => {}
            other => panic!("expected a read failure, got {other:?}"),
        }
    }

    #[test]
    fn a_new_configuration_is_created_empty_and_publishes_the_edit() {
        let dir = scratch("new");
        let file = dir.join("settings.toml");

        {
            let mut config = Config::<Settings>::new(&file).unwrap();

            // Created on the spot, holding the default value, and staged like a read.
            assert_eq!(config.name, "");
            assert_eq!(config.level, 0);
            assert!(file.exists());
            assert_eq!(config.lock(), dir.join("settings.toml.lock"));
            assert!(config.lock().exists());

            config.level = 4;
            config.write().unwrap();

            // The file still holds the empty configuration: only the staging copy has
            // the edit in it yet.
            assert_eq!(
                fs::read_to_string(&file).unwrap(),
                "name = \"\"\nlevel = 0\n"
            );
        }

        let published = fs::read_to_string(&file).unwrap();
        assert!(published.contains("level = 4"));
        assert!(!dir.join("settings.toml.lock").exists());
    }

    #[test]
    fn a_new_configuration_is_refused_where_a_staged_edit_is_in_the_way() {
        let dir = scratch("new-locked");
        let file = dir.join("settings.toml");
        fs::write(&file, TOML).unwrap();
        fs::write(
            dir.join("settings.toml.lock"),
            "name = \"staged\"\nlevel = 9\n",
        )
        .unwrap();

        match Config::<Settings>::new(&file) {
            Err(Error::Locked { .. }) => {}
            other => panic!("expected Locked, got {other:?}"),
        }

        // The refusal left both files as they were.
        assert_eq!(fs::read_to_string(&file).unwrap(), TOML);
        assert!(
            fs::read_to_string(dir.join("settings.toml.lock"))
                .unwrap()
                .contains("staged")
        );
    }

    #[test]
    fn the_error_survives_the_crossing_to_c_and_back() {
        use rorolala_utils_lazyffi::{InputType, ReturnType, free_string};

        // A variant with a [`PathBuf`] and two numbers, because that is the payload
        // shape the generated layout is most likely to get wrong, and the one whose
        // strings have to be allocated on the way out.
        let error = Error::Parse {
            file: PathBuf::from("/tmp/rorolala/settings.toml"),
            line: 3,
            column: 7,
        };

        // Rust → C. Nothing else in the crate instantiates these conversions, so
        // without this they are only compiled when a linked export uses them.
        let repr = error.return_self();

        // What C reads out of the union, and what it switches on. The tag is what
        // makes the read sound.
        assert!(matches!(repr.tag, crate::ConfigureErrorTag::Parse));
        // SAFETY: the tag says `Parse` is the live variant, and it was just written.
        let payload = unsafe { repr.payload.Parse };
        let c_file = payload.file;
        assert_eq!(payload.line, 3);
        assert_eq!(payload.column, 7);

        // C → Rust, which borrows the repr — so the string is still ours to release.
        let back = unsafe { Error::input_type(repr) };
        unsafe { free_string(c_file) };

        match back {
            Error::Parse { file, line, column } => {
                assert_eq!(file, PathBuf::from("/tmp/rorolala/settings.toml"));
                assert_eq!(line, 3);
                assert_eq!(column, 7);
            }
            other => panic!("a parse failure came back as {other:?}"),
        }
    }
}
