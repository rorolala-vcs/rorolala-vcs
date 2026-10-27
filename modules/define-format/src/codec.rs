//! Reading and writing a lock's text.

use std::collections::BTreeSet;
use std::str::SplitWhitespace;

use crate::FORMAT_VERSION;
use crate::error::{FormatError, FormatErrorKind};
use crate::hash::ContentHash;
use crate::lock::{Lock, Material, Vault};
use crate::name::VaultName;
use crate::path::LockPath;

/// What a lock's text has been read into so far.
///
/// The sets are what refuses a second Vault or a second material before the lock is built, so the
/// line each is refused on is the line the duplicate is on rather than the end of the file.
#[derive(Default)]
struct Reading {
    /// The format version, once one has been read.
    format: Option<u32>,
    /// The Vaults read so far, in the order they were written.
    vaults: Vec<Vault>,
    /// The materials read so far, in the order they were written.
    materials: Vec<Material>,
    /// The names the Vaults read so far carry.
    names: BTreeSet<String>,
    /// The paths the materials read so far are placed at.
    paths: BTreeSet<String>,
}

impl Lock {
    /// Reads the text a `rola.lock` holds.
    ///
    /// A carriage return before a newline is accepted, because a checkout on Windows may put one
    /// there; everything else is read strictly, since a lock that is half understood is worse than
    /// one that is refused.
    ///
    /// # Errors
    ///
    /// Returns a [`FormatError`] naming the line it was met on when the text is not a lock this
    /// build reads: no format version, a line of a kind the grammar does not name, a fact it does
    /// not read, a malformed hash or path, and a duplicate Vault or path.
    pub fn decode(text: &str) -> Result<Self, FormatError> {
        let mut reading = Reading::default();

        for (at, raw) in text.split('\n').enumerate() {
            let line_number = at + 1;
            let line = raw.strip_suffix('\r').unwrap_or(raw);

            if line.trim().is_empty() {
                continue;
            }

            let Some(first) = line.chars().next() else {
                continue;
            };

            match first {
                '#' => {}
                '?' => read_fact(&mut reading, line_number, &line[1..])?,
                '/' | '\\' => read_material(&mut reading, line_number, line)?,
                other => {
                    return Err(FormatError::at(
                        line_number,
                        FormatErrorKind::UnknownLine { first: other },
                    ));
                }
            }
        }

        if reading.format.is_none() {
            return Err(FormatError::plain(FormatErrorKind::MissingFormat));
        }

        Self::new(reading.vaults, reading.materials)
    }

    /// Writes the lock as the text of a `rola.lock`.
    ///
    /// The spelling is one: the format first, the Vaults in the order they were written, the
    /// materials by path, every line ended with a newline. So two locks holding the same thing are
    /// the same bytes.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut text = String::new();

        text.push_str("? FORMAT ");
        text.push_str(&self.format().to_string());
        text.push('\n');

        for vault in self.vaults() {
            text.push_str("? VAULT ");
            text.push_str(vault.name().as_str());
            text.push(' ');
            text.push_str(vault.address());
            text.push('\n');
        }

        for material in self.materials() {
            text.push('/');
            text.push_str(material.path().as_str());
            text.push(' ');
            text.push_str(&material.content().to_token());
            text.push('\n');
        }

        text
    }
}

/// Reads one `?` line into `reading`.
fn read_fact(reading: &mut Reading, line_number: usize, body: &str) -> Result<(), FormatError> {
    let mut words = body.split_whitespace();
    let Some(name) = words.next() else {
        return Err(malformed(line_number, "?"));
    };

    if name == "FORMAT" {
        return read_format(reading, line_number, &mut words);
    }

    // Everything else is a fact of a lock that has already named its format.
    if reading.format.is_none() {
        return Err(FormatError::at(
            line_number,
            FormatErrorKind::FormatNotFirst,
        ));
    }

    if name == "VAULT" {
        return read_vault(reading, line_number, &mut words);
    }

    Err(FormatError::at(
        line_number,
        FormatErrorKind::UnknownFact {
            name: name.to_owned(),
        },
    ))
}

/// Reads the words of a `? FORMAT` line into `reading`.
fn read_format(
    reading: &mut Reading,
    line_number: usize,
    words: &mut SplitWhitespace<'_>,
) -> Result<(), FormatError> {
    if reading.format.is_some() {
        return Err(FormatError::at(
            line_number,
            FormatErrorKind::RepeatedFormat,
        ));
    }

    let Some(version) = words.next() else {
        return Err(malformed(line_number, "FORMAT"));
    };

    if words.next().is_some() {
        return Err(malformed(line_number, "FORMAT"));
    }

    let version = version.parse::<u32>().map_err(|_| {
        FormatError::at(
            line_number,
            FormatErrorKind::MalformedFormat {
                text: version.to_owned(),
            },
        )
    })?;

    if version != FORMAT_VERSION {
        return Err(FormatError::at(
            line_number,
            FormatErrorKind::UnsupportedFormat { version },
        ));
    }

    reading.format = Some(version);

    Ok(())
}

/// Reads the words of a `? VAULT` line into `reading`.
fn read_vault(
    reading: &mut Reading,
    line_number: usize,
    words: &mut SplitWhitespace<'_>,
) -> Result<(), FormatError> {
    let Some(name) = words.next() else {
        return Err(malformed(line_number, "VAULT"));
    };

    let Some(address) = words.next() else {
        return Err(malformed(line_number, "VAULT"));
    };

    if words.next().is_some() {
        return Err(malformed(line_number, "VAULT"));
    }

    let name = VaultName::new(name).map_err(|error| error.with_line(line_number))?;
    let vault = Vault::new(name, address).map_err(|error| error.with_line(line_number))?;

    if !reading.names.insert(vault.name().as_str().to_owned()) {
        return Err(FormatError::at(
            line_number,
            FormatErrorKind::DuplicateVault {
                name: vault.name().as_str().to_owned(),
            },
        ));
    }

    reading.vaults.push(vault);

    Ok(())
}

/// Reads one material line into `reading`.
fn read_material(reading: &mut Reading, line_number: usize, line: &str) -> Result<(), FormatError> {
    if reading.format.is_none() {
        return Err(FormatError::at(
            line_number,
            FormatErrorKind::FormatNotFirst,
        ));
    }

    // The hash is the last word of the line, so the path may hold spaces: it is everything before
    // that word, with the one run of spaces between them cut.
    let trimmed = line.trim_end();
    let Some((path, token)) = trimmed.rsplit_once(|character: char| character.is_whitespace())
    else {
        return Err(FormatError::at(
            line_number,
            FormatErrorKind::MalformedMaterial,
        ));
    };

    let path = path.trim_end();
    let path = LockPath::new(path).map_err(|error| error.with_line(line_number))?;
    let content = ContentHash::from_token(token).map_err(|error| error.with_line(line_number))?;

    if !reading.paths.insert(path.as_str().to_owned()) {
        return Err(FormatError::at(
            line_number,
            FormatErrorKind::DuplicatePath {
                path: path.as_str().to_owned(),
            },
        ));
    }

    reading.materials.push(Material::new(path, content));

    Ok(())
}

/// A `?` line that does not carry the words the fact it names needs.
fn malformed(line_number: usize, name: &str) -> FormatError {
    FormatError::at(
        line_number,
        FormatErrorKind::MalformedFact {
            name: name.to_owned(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::Lock;
    use crate::error::FormatErrorKind;
    use crate::hash::ContentHash;
    use crate::lock::{Material, Vault};
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

    /// The lock most tests read and write.
    fn lock() -> Lock {
        Lock::new(
            vec![vault("origin", "rola://127.0.0.1:7717/default")],
            vec![
                material("/Source/character.fbx", 2),
                material("/Assets/hero.psd", 1),
            ],
        )
        .unwrap()
    }

    /// The digest of `byte`, as a lock writes it.
    fn token(byte: u8) -> String {
        ContentHash::from_digest([byte; 32]).to_token()
    }

    #[test]
    fn a_lock_written_down_reads_back_as_it_went_in() {
        let lock = lock();
        let text = lock.encode();

        assert_eq!(
            text,
            format!(
                "? FORMAT 1\n? VAULT origin rola://127.0.0.1:7717/default\n/Assets/hero.psd {}\n/Source/character.fbx {}\n",
                token(1),
                token(2)
            )
        );
        assert_eq!(Lock::decode(&text).unwrap(), lock);
    }

    #[test]
    fn one_set_of_materials_has_one_spelling() {
        let first = Lock::new(vec![], vec![material("/b.txt", 2), material("/a.txt", 1)]).unwrap();
        let second = Lock::new(vec![], vec![material("/a.txt", 1), material("/b.txt", 2)]).unwrap();

        assert_eq!(first.encode(), second.encode());
        assert_eq!(first, second);
    }

    #[test]
    fn a_carriage_return_before_a_newline_is_accepted_and_not_written() {
        let lock = lock();
        let windows = lock.encode().replace('\n', "\r\n");

        let read = Lock::decode(&windows).unwrap();

        assert_eq!(read, lock);
        assert!(!read.encode().contains('\r'));
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let text = format!(
            "# a lock written by hand\n\n? FORMAT 1\n\n# the source\n? VAULT origin rola://a/default\n\n/Assets/hero.psd {}\n",
            token(1)
        );

        let read = Lock::decode(&text).unwrap();

        assert_eq!(read.vaults().len(), 1);
        assert_eq!(read.materials().len(), 1);
    }

    #[test]
    fn a_lock_with_no_format_is_refused() {
        for text in ["", "\n\n", "# only a comment\n"] {
            let error = Lock::decode(text).unwrap_err();

            assert!(
                matches!(error.kind(), FormatErrorKind::MissingFormat),
                "{text:?}"
            );
            assert_eq!(error.line(), None);
        }
    }

    #[test]
    fn a_value_before_the_format_is_refused() {
        for text in [
            "? VAULT origin rola://a/default\n? FORMAT 1\n".to_owned(),
            format!("/Assets/hero.psd {}\n? FORMAT 1\n", token(1)),
        ] {
            let error = Lock::decode(&text).unwrap_err();

            assert!(
                matches!(error.kind(), FormatErrorKind::FormatNotFirst),
                "{text:?}"
            );
            assert_eq!(error.line(), Some(1));
        }
    }

    #[test]
    fn a_format_named_twice_is_refused() {
        let error = Lock::decode("? FORMAT 1\n? FORMAT 1\n").unwrap_err();

        assert!(matches!(error.kind(), FormatErrorKind::RepeatedFormat));
        assert_eq!(error.line(), Some(2));
    }

    #[test]
    fn a_format_this_build_does_not_read_is_refused() {
        let error = Lock::decode("? FORMAT 2\n").unwrap_err();

        assert!(matches!(
            error.kind(),
            FormatErrorKind::UnsupportedFormat { version: 2 }
        ));
        assert_eq!(error.line(), Some(1));
    }

    #[test]
    fn a_format_that_is_not_a_number_is_refused() {
        let error = Lock::decode("? FORMAT one\n").unwrap_err();

        assert!(matches!(
            error.kind(),
            FormatErrorKind::MalformedFormat { text } if text == "one"
        ));
        assert_eq!(error.line(), Some(1));
    }

    #[test]
    fn a_fact_this_build_does_not_read_is_refused() {
        let error = Lock::decode("? FORMAT 1\n? THING x\n").unwrap_err();

        assert!(matches!(
            error.kind(),
            FormatErrorKind::UnknownFact { name } if name == "THING"
        ));
        assert_eq!(error.line(), Some(2));
    }

    #[test]
    fn a_fact_without_the_words_it_needs_is_refused() {
        for text in [
            "? FORMAT 1\n? VAULT\n",
            "? FORMAT 1\n? VAULT origin\n",
            "? FORMAT 1\n? VAULT origin rola://a/default extra\n",
            "? FORMAT 1\n?\n",
        ] {
            let error = Lock::decode(text).unwrap_err();

            assert!(
                matches!(error.kind(), FormatErrorKind::MalformedFact { .. }),
                "{text:?}"
            );
            assert_eq!(error.line(), Some(2));
        }
    }

    #[test]
    fn a_line_of_a_kind_the_grammar_does_not_name_is_refused() {
        let error = Lock::decode("? FORMAT 1\nunsaid\n").unwrap_err();

        assert!(matches!(
            error.kind(),
            FormatErrorKind::UnknownLine { first: 'u' }
        ));
        assert_eq!(error.line(), Some(2));
    }

    #[test]
    fn a_material_line_without_a_hash_is_refused() {
        let error = Lock::decode("? FORMAT 1\n/Assets/hero.psd\n").unwrap_err();

        assert!(matches!(error.kind(), FormatErrorKind::MalformedMaterial));
        assert_eq!(error.line(), Some(2));
    }

    #[test]
    fn a_material_line_whose_hash_is_not_a_content_hash_is_refused() {
        let error = Lock::decode("? FORMAT 1\n/Assets/hero.psd nope\n").unwrap_err();

        assert!(matches!(
            error.kind(),
            FormatErrorKind::MalformedHash { text } if text == "nope"
        ));
        assert_eq!(error.line(), Some(2));
    }

    #[test]
    fn a_material_path_that_cannot_be_one_place_is_refused() {
        let error = Lock::decode(&format!("? FORMAT 1\n/../x {}\n", token(1))).unwrap_err();

        assert!(matches!(error.kind(), FormatErrorKind::InvalidPath { .. }));
        assert_eq!(error.line(), Some(2));
    }

    #[test]
    fn two_vaults_with_one_name_are_refused_with_the_line_they_meet_on() {
        let error = Lock::decode(
            "? FORMAT 1\n? VAULT origin rola://a/default\n? VAULT origin rola://b/default\n",
        )
        .unwrap_err();

        assert!(matches!(
            error.kind(),
            FormatErrorKind::DuplicateVault { name } if name == "origin"
        ));
        assert_eq!(error.line(), Some(3));
    }

    #[test]
    fn two_materials_at_one_path_are_refused_with_the_line_they_meet_on() {
        let error = Lock::decode(&format!(
            "? FORMAT 1\n/Assets/hero.psd {}\n/Assets/hero.psd {}\n",
            token(1),
            token(2)
        ))
        .unwrap_err();

        assert!(matches!(
            error.kind(),
            FormatErrorKind::DuplicatePath { path } if path == "Assets/hero.psd"
        ));
        assert_eq!(error.line(), Some(3));
    }

    #[test]
    fn a_material_path_may_hold_spaces() {
        let read = Lock::decode(&format!(
            "? FORMAT 1\n/Program Files/Some Game/Data.pak {}\n",
            token(3)
        ))
        .unwrap();

        assert_eq!(
            read.materials()[0].path().as_str(),
            "Program Files/Some Game/Data.pak"
        );
    }

    #[test]
    fn an_empty_lock_round_trips() {
        let empty = Lock::empty();

        assert_eq!(empty.encode(), "? FORMAT 1\n");
        assert_eq!(Lock::decode(&empty.encode()).unwrap(), empty);
    }

    #[test]
    fn a_lock_with_nothing_but_a_format_round_trips() {
        let read = Lock::decode("? FORMAT 1\n").unwrap();

        assert_eq!(read, Lock::empty());
    }
}
