//! Reading the articles out of a book directory.
//!
//! An article is a Markdown file with an optional YAML front matter block. Two files never meet
//! unless one is under the other's directory: the walk is over every `*.md` under `book/`, and a
//! file or directory whose name begins with `__` is left out — that is where this tree keeps a
//! draft, and a draft is not part of the book that ships.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use rorolala_book_search::SourceArticle;
use serde::Deserialize;

use crate::Error;
use crate::markdown;

/// The front matter block an article may begin with.
///
/// Only the three keys the search reads are named; a block that carries more is read for those
/// three and the rest is left where it is, because the front matter belongs to whoever writes the
/// article and not to the index.
#[derive(Debug, Default, Deserialize)]
struct FrontMatter {
    /// The title a hit is shown by.
    title: Option<String>,
    /// The description a hit is shown with.
    description: Option<String>,
    /// The keywords the article is found by: one comma-separated line, or a YAML list.
    keywords: Option<Keywords>,
}

/// The two ways a keyword list is written.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Keywords {
    /// One comma-separated line, which is what the article template carries.
    Line(String),
    /// A YAML list.
    List(Vec<String>),
}

impl Keywords {
    /// The keywords, trimmed and without the empty ones.
    fn into_list(self) -> Vec<String> {
        let words: Vec<String> = match self {
            Self::Line(line) => line.split(',').map(str::to_owned).collect(),
            Self::List(list) => list,
        };

        words
            .into_iter()
            .map(|word| word.trim().to_owned())
            .filter(|word| !word.is_empty())
            .collect()
    }
}

/// Every article under `book`, in name order.
///
/// # Errors
///
/// Reports [`Error`] when the directory cannot be walked, when a file cannot be read or is not
/// UTF-8, when a front matter block is not YAML, or when there is no article to read at all.
pub fn read(book: &Path) -> Result<Vec<SourceArticle>, Error> {
    let mut files = Vec::new();

    if book.is_dir() {
        collect(book, Path::new(""), &mut files)?;
    }

    if files.is_empty() {
        return Err(Error::NoArticles {
            path: book.to_owned(),
        });
    }

    let mut articles = Vec::with_capacity(files.len());
    for relative in files {
        articles.push(article(book, &relative)?);
    }

    Ok(articles)
}

/// Walks `directory`, pushing every `*.md` under it as a path relative to `book`.
fn collect(directory: &Path, relative: &Path, found: &mut Vec<PathBuf>) -> Result<(), Error> {
    let entries = std::fs::read_dir(directory).map_err(|error| Error::Io {
        doing: "reading the book's directory",
        detail: format!("{}: {error}", directory.display()),
    })?;

    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| Error::Io {
            doing: "reading an entry of the book's directory",
            detail: format!("{}: {error}", directory.display()),
        })?;
        paths.push(entry.path());
    }
    paths.sort();

    for path in paths {
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            return Err(Error::Io {
                doing: "naming an article",
                detail: format!("{}: the file name is not text", path.display()),
            });
        };

        // A draft is parked under `__` and is not part of the book, the way `.gitignore` says.
        if name.starts_with("__") {
            continue;
        }

        let child = relative.join(name);

        if path.is_dir() {
            collect(&path, &child, found)?;
        } else if is_markdown(name) {
            found.push(child);
        }
    }

    Ok(())
}

/// Whether a file name is a Markdown one.
fn is_markdown(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

/// Reads one article.
fn article(book: &Path, relative: &Path) -> Result<SourceArticle, Error> {
    let path = book.join(relative);
    let bytes = std::fs::read(&path).map_err(|error| Error::Io {
        doing: "reading an article",
        detail: format!("{}: {error}", path.display()),
    })?;

    let text = String::from_utf8(bytes).map_err(|error| Error::Utf8 {
        path: path.clone(),
        detail: error.to_string(),
    })?;

    let text = normalized(&text);
    let (front, body) = split(&text);
    let front: FrontMatter = match front {
        None => FrontMatter::default(),
        Some(front) => serde_yaml::from_str(front).map_err(|error| Error::FrontMatter {
            path: path.clone(),
            detail: error.to_string(),
        })?,
    };

    let name = name_of(relative).ok_or_else(|| Error::Io {
        doing: "naming an article",
        detail: format!("{}: the path is not text", path.display()),
    })?;

    let description = front
        .description
        .map_or_else(String::new, |description| description.trim().to_owned());
    let keywords = front.keywords.map_or_else(Vec::new, Keywords::into_list);
    let markdown = body.to_owned();
    let plain = markdown::plain(body);
    let title = front
        .title
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty())
        .or_else(|| markdown::title(body))
        .unwrap_or_else(|| {
            name.rsplit('/')
                .next()
                .map_or_else(|| name.clone(), str::to_owned)
        });

    Ok(SourceArticle {
        name,
        title,
        description,
        keywords,
        markdown,
        plain,
    })
}

/// The text with the line endings the format uses and no byte-order mark.
fn normalized(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");

    text.strip_prefix('\u{feff}')
        .map_or_else(|| text.clone(), str::to_owned)
}

/// Splits an article into its front matter and its body.
///
/// A file that does not begin with a front matter block is all body: the block is optional, and a
/// file that begins with something else is an article rather than a file that failed to be one.
fn split(text: &str) -> (Option<&str>, &str) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return (None, text);
    };

    if let Some(at) = rest.find("\n---\n") {
        return (Some(&rest[..at]), &rest[at + "\n---\n".len()..]);
    }

    if let Some(front) = rest.strip_suffix("\n---") {
        return (Some(front), "");
    }

    (None, text)
}

/// The name an article is found by: its path under `book/`, `/`-separated, without the extension.
fn name_of(relative: &Path) -> Option<String> {
    let mut parts = Vec::new();

    for component in relative.components() {
        let Component::Normal(part) = component else {
            return None;
        };

        parts.push(part.to_str()?);
    }

    let last = parts.pop()?;
    let (stem, _) = last.rsplit_once('.')?;
    parts.push(stem);

    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{name_of, split};

    #[test]
    fn a_front_matter_block_is_taken_off_the_body() {
        let (front, body) = split("---\ntitle: One\n---\n\nContents\n");

        assert_eq!(front, Some("title: One"));
        assert_eq!(body, "\nContents\n");
    }

    #[test]
    fn a_block_that_is_not_there_is_all_body() {
        let (front, body) = split("# One\n\nContents\n");

        assert_eq!(front, None);
        assert_eq!(body, "# One\n\nContents\n");
    }

    #[test]
    fn a_block_that_is_the_whole_file_has_no_body() {
        let (front, body) = split("---\ntitle: One\n---");

        assert_eq!(front, Some("title: One"));
        assert_eq!(body, "");
    }

    #[test]
    fn a_name_is_the_path_under_the_book() {
        assert_eq!(
            name_of(Path::new("getting-started/install.md")),
            Some("getting-started/install".to_owned())
        );
        assert_eq!(name_of(Path::new("install.md")), Some("install".to_owned()));
        assert_eq!(name_of(Path::new("a/b/c.MD")), Some("a/b/c".to_owned()));
    }

    #[test]
    fn a_front_matter_block_that_is_not_yaml_is_refused() {
        let (front, _) = split("---\ntitle: [unclosed\n---\nbody");

        assert!(serde_yaml::from_str::<super::FrontMatter>(front.expect("a block")).is_err());
    }

    #[test]
    fn keywords_are_one_line_or_a_list() {
        let one: super::FrontMatter =
            serde_yaml::from_str("keywords: install, setup").expect("a block");
        assert_eq!(
            one.keywords.expect("keywords").into_list(),
            ["install", "setup"]
        );

        let many: super::FrontMatter =
            serde_yaml::from_str("keywords:\n  - install\n  - setup\n").expect("a block");
        assert_eq!(
            many.keywords.expect("keywords").into_list(),
            ["install", "setup"]
        );
    }
}
