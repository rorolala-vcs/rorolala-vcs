//! The version a workspace manifest states, read and replaced without rewriting the file.
//!
//! The manifest is the tree's own, comments and order included, so it is edited as text rather than
//! parsed and written back: a round trip through a TOML value would lose the reasons the sections
//! carry. Only the inside of the `version` string in `[workspace.package]` is replaced.

use std::path::Path;

use crate::Error;

/// The version `[workspace.package]` states, or nothing when it states none.
///
/// # Errors
///
/// Reports [`Error::Manifest`] when the section names a `version` that is not one quoted string.
pub fn version(path: &Path, text: &str) -> Result<Option<String>, Error> {
    let mut in_section = false;

    for line in text.lines() {
        if let Some(section) = section_name(line) {
            in_section = section == "workspace.package";
            continue;
        }

        if !in_section {
            continue;
        }

        match version_assignment(line) {
            Ok(Some((value, _, _))) => return Ok(Some(value)),
            Ok(None) => {}
            Err(detail) => {
                return Err(Error::Manifest {
                    path: path.to_path_buf(),
                    detail: detail.to_owned(),
                });
            }
        }
    }

    Ok(None)
}

/// `text` with the version in `[workspace.package]` replaced by `version`.
///
/// # Errors
///
/// Reports [`Error::ManifestVersionMissing`] when the section states no literal version, and
/// [`Error::Manifest`] when it states one that cannot be replaced safely.
pub fn set_version(path: &Path, text: &str, version: &str) -> Result<String, Error> {
    let mut in_section = false;
    let mut replaced = false;
    let mut output = String::with_capacity(text.len() + version.len());

    for line in text.lines() {
        if let Some(section) = section_name(line) {
            in_section = section == "workspace.package";
            output.push_str(line);
            output.push('\n');
            continue;
        }

        if in_section && !replaced {
            match version_assignment(line) {
                Ok(Some((_, start, end))) => {
                    output.push_str(&line[..start]);
                    output.push_str(version);
                    output.push_str(&line[end..]);
                    output.push('\n');
                    replaced = true;
                    continue;
                }
                Ok(None) => {}
                Err(detail) => {
                    return Err(Error::Manifest {
                        path: path.to_path_buf(),
                        detail: detail.to_owned(),
                    });
                }
            }
        }

        output.push_str(line);
        output.push('\n');
    }

    if !replaced {
        return Err(Error::ManifestVersionMissing {
            path: path.to_path_buf(),
        });
    }

    Ok(output)
}

/// The table a line opens, when it opens one.
fn section_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix('[')?;
    let end = rest.find(']')?;

    Some(rest[..end].trim())
}

/// The version assignment a line makes: its value and where that value sits.
fn version_assignment(line: &str) -> Result<Option<(String, usize, usize)>, &'static str> {
    let Some(equals) = line.find('=') else {
        return Ok(None);
    };

    if line[..equals].trim() != "version" {
        return Ok(None);
    }

    let after = &line[equals + 1..];
    let Some(start) = after.find(|character: char| !character.is_whitespace()) else {
        return Err("`version` has no value");
    };

    let value = &after[start..];
    let quote = value.chars().next().ok_or("`version` has no value")?;

    if !matches!(quote, '"' | '\'') {
        return Err("`version` is not a quoted string");
    }

    let rest = &value[1..];
    let end = rest.find(quote).ok_or("`version` has no closing quote")?;
    let inner = &rest[..end];

    if inner.trim().is_empty() {
        return Err("`version` is empty");
    }

    let trailing = rest[end + 1..].trim();
    if !trailing.is_empty() && !trailing.starts_with('#') {
        return Err("`version` has text after its quoted value");
    }

    let value_start = equals + 1 + start + 1;

    Ok(Some((
        inner.to_owned(),
        value_start,
        value_start + inner.len(),
    )))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{set_version, version};

    const MANIFEST: &str = "Cargo.toml";

    const SAMPLE: &str = "\
[workspace]
members = [\"a\"]

[workspace.package]
license = \"MIT\"
version = \"0.1.0\" # the one that moves
edition = \"2024\"

[package]
name = \"root\"
version.workspace = true
";

    #[test]
    fn the_workspace_version_is_the_one_read() {
        let read = version(Path::new(MANIFEST), SAMPLE).unwrap();
        assert_eq!(read.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn a_package_version_is_not_the_workspace_version() {
        let text = "[package]\nname = \"a\"\nversion = \"9.9.9\"\n";
        assert_eq!(version(Path::new(MANIFEST), text).unwrap(), None);
    }

    #[test]
    fn only_the_workspace_version_moves_and_the_comment_stays() {
        let updated = set_version(Path::new(MANIFEST), SAMPLE, "0.2.0").unwrap();

        assert!(updated.contains("version = \"0.2.0\" # the one that moves"));
        assert!(updated.contains("version.workspace = true"));
        assert_eq!(
            version(Path::new(MANIFEST), &updated).unwrap().as_deref(),
            Some("0.2.0")
        );
    }

    #[test]
    fn a_single_quoted_version_is_read_too() {
        let text = "[workspace.package]\nversion = '1.2.3'\n";
        assert_eq!(
            version(Path::new(MANIFEST), text).unwrap().as_deref(),
            Some("1.2.3")
        );
    }

    #[test]
    fn a_manifest_without_a_version_has_none_to_replace() {
        let text = "[workspace.package]\nedition = \"2024\"\n";
        assert!(set_version(Path::new(MANIFEST), text, "0.2.0").is_err());
    }

    #[test]
    fn a_version_that_is_not_a_quoted_string_is_not_read_as_one() {
        let text = "[workspace.package]\nversion = 1\n";
        assert!(version(Path::new(MANIFEST), text).is_err());
    }
}
