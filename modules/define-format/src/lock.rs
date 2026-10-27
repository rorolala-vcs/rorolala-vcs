//! The lock itself: the Vaults its content comes from and the materials it places.

use std::collections::BTreeSet;

use crate::FORMAT_VERSION;
use crate::error::{FormatError, FormatErrorKind};
use crate::hash::ContentHash;
use crate::name::VaultName;
use crate::path::LockPath;

/// One Vault a lock's materials can be fetched from.
///
/// A lock writes where content can be asked for, not what an address means: what is kept here is
/// the name a mapping's pin can refer to and the address as it was written, and reading an address
/// is the caller's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vault {
    /// The name the Vault is given in this lock.
    name: VaultName,
    /// Where the Vault is reached, as it was written.
    address: String,
}

impl Vault {
    /// A Vault named `name`, reached at `address`.
    ///
    /// # Errors
    ///
    /// Returns [`FormatErrorKind::InvalidAddress`] when `address` cannot be one word of a line:
    /// empty, or holding whitespace or a control character.
    pub fn new(name: VaultName, address: impl Into<String>) -> Result<Self, FormatError> {
        let address = address.into();

        if address.is_empty()
            || address
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(FormatError::plain(FormatErrorKind::InvalidAddress {
                address,
            }));
        }

        Ok(Self { name, address })
    }

    /// The name the Vault is given.
    #[must_use]
    pub const fn name(&self) -> &VaultName {
        &self.name
    }

    /// Where the Vault is reached, as it was written.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }
}

/// One material: where it is placed and what content is placed there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Material {
    /// Where it is placed, relative to the project's root.
    path: LockPath,
    /// The content placed there.
    content: ContentHash,
}

impl Material {
    /// A material placed at `path`, holding `content`.
    #[must_use]
    pub const fn new(path: LockPath, content: ContentHash) -> Self {
        Self { path, content }
    }

    /// Where it is placed.
    #[must_use]
    pub const fn path(&self) -> &LockPath {
        &self.path
    }

    /// What content is placed there.
    #[must_use]
    pub const fn content(&self) -> ContentHash {
        self.content
    }
}

/// A `rola.lock` read whole: the Vaults it can fetch from and the materials it places.
///
/// The materials are held sorted by path and free of duplicates, so one set of materials has one
/// spelling and [`encode`](Lock::encode) is deterministic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lock {
    /// The format version the lock is in.
    format: u32,
    /// The Vaults content can be fetched from, in the order they were written.
    vaults: Vec<Vault>,
    /// The materials, sorted by path.
    materials: Vec<Material>,
}

impl Lock {
    /// A lock in this build's format holding `vaults` and `materials`.
    ///
    /// # Errors
    ///
    /// Returns [`FormatErrorKind::DuplicateVault`] when two Vaults share a name, and
    /// [`FormatErrorKind::DuplicatePath`] when two materials share a path.
    pub fn new(vaults: Vec<Vault>, materials: Vec<Material>) -> Result<Self, FormatError> {
        let mut names: BTreeSet<&str> = BTreeSet::new();
        for vault in &vaults {
            if !names.insert(vault.name().as_str()) {
                return Err(FormatError::plain(FormatErrorKind::DuplicateVault {
                    name: vault.name().as_str().to_owned(),
                }));
            }
        }

        let mut paths: BTreeSet<&str> = BTreeSet::new();
        for material in &materials {
            if !paths.insert(material.path().as_str()) {
                return Err(FormatError::plain(FormatErrorKind::DuplicatePath {
                    path: material.path().as_str().to_owned(),
                }));
            }
        }

        let mut materials = materials;
        materials.sort_by(|left, right| left.path().cmp(right.path()));

        Ok(Self {
            format: FORMAT_VERSION,
            vaults,
            materials,
        })
    }

    /// A lock naming no Vault and placing no material.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            format: FORMAT_VERSION,
            vaults: Vec::new(),
            materials: Vec::new(),
        }
    }

    /// The format version the lock is in.
    #[must_use]
    pub const fn format(&self) -> u32 {
        self.format
    }

    /// The Vaults content can be fetched from, in the order they were written.
    #[must_use]
    pub fn vaults(&self) -> &[Vault] {
        &self.vaults
    }

    /// The materials, sorted by path.
    #[must_use]
    pub fn materials(&self) -> &[Material] {
        &self.materials
    }
}

#[cfg(test)]
mod tests {
    use super::{Lock, Material, Vault};
    use crate::error::FormatErrorKind;
    use crate::hash::ContentHash;
    use crate::name::VaultName;
    use crate::path::LockPath;

    /// A path, from text a test can read.
    fn path(text: &str) -> LockPath {
        LockPath::new(text).unwrap()
    }

    /// A Vault, from text a test can read.
    fn vault(name: &str, address: &str) -> Vault {
        Vault::new(VaultName::new(name).unwrap(), address).unwrap()
    }

    /// A material at `path_text`, holding content `byte`.
    fn material(path_text: &str, byte: u8) -> Material {
        Material::new(path(path_text), ContentHash::from_digest([byte; 32]))
    }

    #[test]
    fn a_lock_sorts_its_materials_by_path_and_keeps_its_vaults_in_order() {
        let lock = Lock::new(
            vec![
                vault("origin", "rola://a/default"),
                vault("mirror", "rola://b/default"),
            ],
            vec![
                material("/Source/character.fbx", 2),
                material("/Assets/hero.psd", 1),
            ],
        )
        .unwrap();

        let paths: Vec<&str> = lock
            .materials()
            .iter()
            .map(|material| material.path().as_str())
            .collect();
        let names: Vec<&str> = lock
            .vaults()
            .iter()
            .map(|vault| vault.name().as_str())
            .collect();

        assert_eq!(paths, ["Assets/hero.psd", "Source/character.fbx"]);
        assert_eq!(names, ["origin", "mirror"]);
        assert_eq!(lock.format(), crate::FORMAT_VERSION);
    }

    #[test]
    fn a_lock_with_no_vault_and_no_material_is_empty() {
        let lock = Lock::empty();

        assert_eq!(lock, Lock::new(Vec::new(), Vec::new()).unwrap());
        assert_eq!(lock.format(), crate::FORMAT_VERSION);
    }

    #[test]
    fn two_vaults_with_one_name_are_refused() {
        let error = Lock::new(
            vec![
                vault("origin", "rola://a/default"),
                vault("origin", "rola://b/default"),
            ],
            vec![],
        )
        .unwrap_err();

        assert!(
            matches!(error.kind(), FormatErrorKind::DuplicateVault { name } if name == "origin")
        );
        assert_eq!(
            error.line(),
            None,
            "a caller's value is not a line of a file"
        );
    }

    #[test]
    fn two_materials_at_one_path_are_refused() {
        let error = Lock::new(
            vec![],
            vec![
                material("/Assets/hero.psd", 1),
                material("/Assets/hero.psd", 2),
            ],
        )
        .unwrap_err();

        assert!(
            matches!(error.kind(), FormatErrorKind::DuplicatePath { path } if path == "Assets/hero.psd")
        );
    }

    #[test]
    fn an_address_that_cannot_be_one_word_of_a_line_is_refused() {
        let name = VaultName::new("origin").unwrap();

        for address in ["", "rola://a b/default", "rola://a\nb/default"] {
            let error = Vault::new(name.clone(), address).unwrap_err();
            assert!(
                matches!(error.kind(), FormatErrorKind::InvalidAddress { .. }),
                "{address:?}"
            );
        }

        assert!(Vault::new(name, "rola://127.0.0.1:7717/default").is_ok());
    }
}
