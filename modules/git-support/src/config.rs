//! The `rola.toml` a project is described by.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use uuid::Uuid;

use rola_define_format::LockPath;

use crate::error::Error;

/// A project's file: the Vaults its materials come from, what the features do, and one mapping per
/// material.
///
/// It is the file a person writes, so it is read the way a hand-written file is: a key this build
/// does not know is refused rather than ignored, since a spelling that quietly does nothing is a
/// project that quietly does not do what it says.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    /// The Vaults the project knows, each under a name of its own.
    #[serde(default)]
    pub vaults: BTreeMap<String, String>,
    /// What the project asks git support to do.
    #[serde(default)]
    pub features: Features,
    /// What a mapping means when it says nothing itself.
    #[serde(default)]
    pub defaults: Defaults,
    /// The materials the project places.
    #[serde(default, rename = "mapping")]
    pub mapping: Vec<Mapping>,
}

impl ProjectConfig {
    /// Reads the project file at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the file cannot be read, [`Error::Config`] when it is not TOML
    /// this build reads, and [`Error::Format`] or [`Error::ConfigValue`] when a mapping names
    /// something a lock could not name or the same material twice.
    pub fn read(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path).map_err(Error::Io)?;
        let config: Self = toml::from_str(&text).map_err(Error::Config)?;

        config.validate()?;

        Ok(config)
    }

    /// Whether the project is kept in sync with git.
    #[must_use]
    pub const fn git_support(&self) -> bool {
        self.features.git_support
    }

    /// The address of the Vault named `name`, if the project knows it.
    #[must_use]
    pub fn vault(&self, name: &str) -> Option<&str> {
        self.vaults.get(name).map(String::as_str)
    }

    /// The Vault a `latest` pin resolves through: the one named as the default, or the first named
    /// when none is.
    #[must_use]
    pub fn default_vault(&self) -> Option<&str> {
        self.defaults
            .vault
            .as_deref()
            .or_else(|| self.vaults.keys().next().map(String::as_str))
    }

    /// Refuses a project that names one material twice or one place twice.
    fn validate(&self) -> Result<(), Error> {
        let mut identifiers: BTreeSet<Uuid> = BTreeSet::new();
        let mut paths: BTreeSet<String> = BTreeSet::new();

        for mapping in &self.mapping {
            if !identifiers.insert(mapping.from) {
                return Err(Error::ConfigValue(format!(
                    "`{}` is mapped more than once",
                    mapping.from
                )));
            }

            let path = mapping.path()?;
            if !paths.insert(path.as_str().to_owned()) {
                return Err(Error::ConfigValue(format!(
                    "`{}` is the place of more than one material",
                    path.as_str()
                )));
            }
        }

        Ok(())
    }
}

/// What the project asks git support to do.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Features {
    /// Whether the project is kept in sync with git: the hooks, the exclude block and the
    /// wrapper all read this.
    #[serde(default)]
    pub git_support: bool,
}

/// What a mapping means when it says nothing itself.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    /// The cut a material is written with when nothing says otherwise.
    #[serde(default)]
    pub policy: Option<String>,
    /// How many requests the resolver may have in flight.
    #[serde(default)]
    pub concurrency: Option<u32>,
    /// The Vault a `latest` pin resolves through, when one is named.
    #[serde(default)]
    pub vault: Option<String>,
}

/// One material the project places.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    /// The UUID the material is known by upstream.
    pub from: Uuid,
    /// Where the material is placed, relative to the project's root.
    pub to: String,
    /// Which version is placed; `latest` when the line says nothing.
    #[serde(default)]
    pub pin: Pin,
}

impl Mapping {
    /// Where the material is placed, as a path a lock may name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Format`] when `to` is not a path a lock may name.
    pub fn path(&self) -> Result<LockPath, Error> {
        LockPath::new(&self.to).map_err(Error::Format)
    }
}

/// Which version of a material is placed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Pin {
    /// The version the default Vault's truth layout names for the material; what an absent pin
    /// means.
    #[default]
    Latest,
    /// A version number in the material's chain.
    Version(u64),
    /// The version a named Vault's truth layout names for the material.
    Vault(String),
}

impl fmt::Display for Pin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Latest => formatter.write_str("latest"),
            Self::Version(version) => write!(formatter, "{version}"),
            Self::Vault(name) => formatter.write_str(name),
        }
    }
}

impl<'de> Deserialize<'de> for Pin {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// Reads a pin as the three shapes a project file may write it in.
        struct Reads;

        impl Visitor<'_> for Reads {
            type Value = Pin;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a version number, `latest`, or the name of a Vault")
            }

            fn visit_u64<E>(self, version: u64) -> Result<Pin, E>
            where
                E: de::Error,
            {
                Ok(Pin::Version(version))
            }

            fn visit_i64<E>(self, version: i64) -> Result<Pin, E>
            where
                E: de::Error,
            {
                u64::try_from(version)
                    .map(Pin::Version)
                    .map_err(|_| E::custom("a version number is not negative"))
            }

            fn visit_str<E>(self, name: &str) -> Result<Pin, E>
            where
                E: de::Error,
            {
                if name == "latest" {
                    Ok(Pin::Latest)
                } else {
                    Ok(Pin::Vault(name.to_owned()))
                }
            }
        }

        deserializer.deserialize_any(Reads)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Pin, ProjectConfig};
    use crate::error::Error;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-git-support-config-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A project file holding `text`, read back.
    fn read(text: &str) -> Result<ProjectConfig, Error> {
        let dir = scratch("read");
        let path = dir.join("rola.toml");

        std::fs::write(&path, text).unwrap();
        let read = ProjectConfig::read(&path);

        let _ = std::fs::remove_dir_all(&dir);

        read
    }

    #[test]
    fn a_project_file_names_its_vaults_features_defaults_and_materials() {
        let config = read(
            r#"
[vaults]
origin = "rola://127.0.0.1:7717/default"
mirror = "rola://mirror.example.com/default"

[features]
git_support = true

[defaults]
policy = "psd-basic"
concurrency = 8
vault = "origin"

[[mapping]]
from = "3f2b1c00-9d4e-4a7b-8c11-0e2f4a6b8c9d"
to = "/Assets/hero.psd"
pin = "latest"

[[mapping]]
from = "7a1b2c3d-4e5f-6071-8293-a4b5c6d7e8f9"
to = "/Assets/background/bg.psd"
pin = "mirror"

[[mapping]]
from = "9c8b7a65-4321-4fed-cba9-876543210fed"
to = "/Source/character.fbx"
pin = 12
"#,
        )
        .unwrap();

        assert!(config.git_support());
        assert_eq!(
            config.vault("origin"),
            Some("rola://127.0.0.1:7717/default")
        );
        assert_eq!(config.default_vault(), Some("origin"));
        assert_eq!(config.defaults.policy.as_deref(), Some("psd-basic"));
        assert_eq!(config.defaults.concurrency, Some(8));
        assert_eq!(config.mapping.len(), 3);

        assert_eq!(config.mapping[0].pin, Pin::Latest);
        assert_eq!(config.mapping[1].pin, Pin::Vault("mirror".to_owned()));
        assert_eq!(config.mapping[2].pin, Pin::Version(12));
        assert_eq!(
            config.mapping[2].path().unwrap().as_str(),
            "Source/character.fbx"
        );
    }

    #[test]
    fn a_pin_is_latest_when_the_line_says_nothing() {
        let config = read(
            r#"
[[mapping]]
from = "3f2b1c00-9d4e-4a7b-8c11-0e2f4a6b8c9d"
to = "/Assets/hero.psd"
"#,
        )
        .unwrap();

        assert_eq!(config.mapping[0].pin, Pin::Latest);
    }

    #[test]
    fn a_key_this_build_does_not_know_is_refused() {
        let error = read("[features]\ngit_support = true\nffast = true\n").unwrap_err();

        assert!(matches!(error, Error::Config(_)), "{error}");
    }

    #[test]
    fn a_table_this_build_does_not_know_is_refused() {
        let error = read("[unknown]\ngit_support = true\n").unwrap_err();

        assert!(matches!(error, Error::Config(_)), "{error}");
    }

    #[test]
    fn a_material_mapped_twice_is_refused() {
        let error = read(
            r#"
[[mapping]]
from = "3f2b1c00-9d4e-4a7b-8c11-0e2f4a6b8c9d"
to = "/Assets/hero.psd"

[[mapping]]
from = "3f2b1c00-9d4e-4a7b-8c11-0e2f4a6b8c9d"
to = "/Assets/other.psd"
"#,
        )
        .unwrap_err();

        assert!(matches!(error, Error::ConfigValue(_)), "{error}");
    }

    #[test]
    fn one_place_for_two_materials_is_refused() {
        let error = read(
            r#"
[[mapping]]
from = "3f2b1c00-9d4e-4a7b-8c11-0e2f4a6b8c9d"
to = "/Assets/hero.psd"

[[mapping]]
from = "7a1b2c3d-4e5f-6071-8293-a4b5c6d7e8f9"
to = "/Assets/hero.psd"
"#,
        )
        .unwrap_err();

        assert!(matches!(error, Error::ConfigValue(_)), "{error}");
    }

    #[test]
    fn a_place_a_lock_could_not_name_is_refused() {
        let error = read(
            r#"
[[mapping]]
from = "3f2b1c00-9d4e-4a7b-8c11-0e2f4a6b8c9d"
to = "/../hero.psd"
"#,
        )
        .unwrap_err();

        assert!(matches!(error, Error::Format(_)), "{error}");
    }

    #[test]
    fn a_negative_pin_is_refused() {
        let error = read(
            r#"
[[mapping]]
from = "3f2b1c00-9d4e-4a7b-8c11-0e2f4a6b8c9d"
to = "/Assets/hero.psd"
pin = -1
"#,
        )
        .unwrap_err();

        assert!(matches!(error, Error::Config(_)), "{error}");
    }
}
