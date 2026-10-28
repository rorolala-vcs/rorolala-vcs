//! The changelog: versions, the categories under them, and the entries those hold.
//!
//! A document is kept as the text before its first version plus the versions themselves, so the
//! parts the editor is not writing are carried through untouched. Only the version a `write` adds to
//! and the one an `archive` seals are parsed into categories and entries; the rest is preserved line
//! for line.
//!
//! The shape is the one the project agreed on:
//!
//! ```markdown
//! # 0.1.0 (Unreleased)
//!
//! ## Bugfix
//!
//! ### 1. **\[BREAKING!\]** [`cli`] Summary
//!
//! Content.
//! ```
//!
//! A version writes only the categories it has entries for. Within a category, the entries are
//! rewritten in the order the README states: breaking first, then by the scope hierarchy, then by
//! summary, with the numbers reassigned from one.
//!
//! What is written here is the details: the document a person edits. The root `CHANGELOG.md` is
//! derived from it by [`summary`], which keeps the title, versions and categories and turns each
//! entry into a list item linking to the paragraph it holds in the details.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Write as _;

use crate::Error;

/// The categories a version may hold, in the order they are written.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Category {
    /// Something that was wrong and is now right.
    Bugfix,
    /// Something that worked and now works better.
    Optimized,
    /// Something that was not there and now is.
    Feature,
    /// Something that is shaped differently without changing what it is.
    Refactor,
}

impl Category {
    /// Every category, in the order a version writes them.
    const ALL: [Self; 4] = [Self::Bugfix, Self::Optimized, Self::Feature, Self::Refactor];

    /// The heading a version writes for this category.
    #[must_use]
    pub fn heading(self) -> &'static str {
        match self {
            Self::Bugfix => "Bugfix",
            Self::Optimized => "Optimized",
            Self::Feature => "Feature",
            Self::Refactor => "Refactor",
        }
    }

    /// The category a heading names, if it names one.
    #[must_use]
    pub fn from_heading(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.heading().eq_ignore_ascii_case(text))
    }

    /// The category a draft names.
    ///
    /// Breaking is not a category: a draft says what kind of change it is here and whether it is
    /// incompatible with the `breaking` field, so the four names never grow a fifth spelling.
    ///
    /// # Errors
    ///
    /// Reports [`Error::InvalidCategory`] when the name is not one of the four.
    pub fn from_draft(text: &str) -> Result<Self, Error> {
        Self::from_heading(text.trim()).ok_or_else(|| Error::InvalidCategory {
            value: text.trim().to_owned(),
        })
    }
}

/// One entry under a category.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// Whether the entry is marked breaking.
    pub breaking: bool,
    /// The scopes the change touches, primary first.
    pub scopes: Vec<String>,
    /// The one-line summary in the title.
    pub summary: String,
    /// The prose under the title, as it is written.
    pub body: Vec<String>,
    /// The heading level the entry was written at.
    pub heading_level: usize,
    /// The number the title was written with.
    ///
    /// A summary links to the heading as the details spell it, so the number has to be carried
    /// beside the title rather than recomputed from the entry's place in the category.
    pub number: usize,
}

impl Entry {
    /// The entry a title names, if the title is an entry's.
    fn from_heading(text: &str, heading_level: usize) -> Option<Self> {
        let digits = text
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(text.len());
        if digits == 0 {
            return None;
        }

        let number = text[..digits].parse().ok()?;
        let rest = text[digits..].strip_prefix('.')?;
        let rest = if rest.is_empty() {
            rest
        } else {
            rest.strip_prefix(' ')?
        };
        let rest = rest.trim_start();

        let (breaking, rest) = strip_breaking(rest);
        let (scopes, rest) = parse_scopes(rest);

        Some(Self {
            breaking,
            scopes,
            summary: rest.trim().to_owned(),
            body: Vec::new(),
            heading_level,
            number,
        })
    }

    /// The title after the number: the breaking marker, the scopes and the summary.
    fn title(&self) -> String {
        let mut title = String::new();

        if self.breaking {
            title.push_str("**\\[BREAKING!\\]**");
        }

        for scope in &self.scopes {
            if !title.is_empty() {
                title.push(' ');
            }

            title.push_str("[`");
            title.push_str(scope);
            title.push_str("`]");
        }

        if !self.summary.is_empty() {
            if !title.is_empty() {
                title.push(' ');
            }

            title.push_str(&self.summary);
        }

        title
    }

    /// The anchor of the heading this entry writes, as the details spell it.
    fn anchor(&self) -> String {
        slug(&format!("{}. {}", self.number, self.title()))
    }

    /// The entry as it is written, under `number`.
    fn render(&self, number: usize) -> String {
        let mut title = format!("{} {number}.", "#".repeat(self.heading_level));
        let rest = self.title();

        if !rest.is_empty() {
            title.push(' ');
            title.push_str(&rest);
        }

        let body = self.body.join("\n");
        let body = body.trim();
        if body.is_empty() {
            title
        } else {
            format!("{title}\n\n{body}")
        }
    }

    /// The order entries are written in: breaking first, then scopes, then summary.
    fn compare(&self, other: &Self) -> Ordering {
        other
            .breaking
            .cmp(&self.breaking)
            .then_with(|| compare_scopes(&self.scopes, &other.scopes))
            .then_with(|| {
                self.summary
                    .to_lowercase()
                    .cmp(&other.summary.to_lowercase())
            })
            .then_with(|| self.summary.cmp(&other.summary))
    }
}

/// The status a version is in, and the date it was sealed on.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Status {
    /// Open: `write` may add entries to it.
    Unreleased,
    /// Sealed: the UTC date it was archived on.
    Released(String),
}

impl Status {
    /// The status as the heading writes it.
    fn text(&self) -> &str {
        match self {
            Self::Unreleased => "Unreleased",
            Self::Released(date) => date,
        }
    }
}

/// One version of the changelog.
#[allow(
    clippy::struct_field_names,
    reason = "the field is the version; a name that avoids the struct's own would say less"
)]
#[derive(Clone, Debug)]
struct Version {
    /// The heading level the version is written at.
    level: usize,
    /// The version itself, as the manifest states it.
    version: String,
    /// Whether the version is still open.
    status: Status,
    /// The lines under the heading.
    body: Vec<String>,
}

impl Version {
    /// The heading this version writes.
    fn heading(&self) -> String {
        format!(
            "{} {} ({})",
            "#".repeat(self.level),
            self.version,
            self.status.text()
        )
    }
}

/// A changelog file.
#[derive(Clone, Debug, Default)]
pub struct Document {
    /// The text before the first version: a title, an introduction, anything carried through.
    before: Vec<String>,
    /// The versions, in the order they are written.
    versions: Vec<Version>,
}

impl Document {
    /// Reads `text` as a changelog.
    ///
    /// A version is a heading ending in `(Unreleased)` or `(YYYY-MM-DD)`. A version-like heading at
    /// a level deeper than the version it appears under is part of that version's body rather than a
    /// new version.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut before = Vec::new();
        let mut versions: Vec<Version> = Vec::new();

        for line in text.lines() {
            let line = line.trim_end().to_owned();

            if let Some((level, version, status)) = version_heading(&line)
                && versions.last().is_none_or(|current| level <= current.level)
            {
                versions.push(Version {
                    level,
                    version,
                    status,
                    body: Vec::new(),
                });
                continue;
            }

            if let Some(current) = versions.last_mut() {
                current.body.push(line);
            } else {
                before.push(line);
            }
        }

        Self { before, versions }
    }

    /// The document as it is written.
    #[must_use]
    pub fn render(&self) -> String {
        let mut blocks = Vec::new();

        let before = self.before.join("\n");
        if !before.trim().is_empty() {
            blocks.push(before.trim().to_owned());
        }

        for version in &self.versions {
            let mut block = version.heading();
            let body = version.body.join("\n");
            let body = body.trim();

            if !body.is_empty() {
                block.push_str("\n\n");
                block.push_str(body);
            }

            blocks.push(block);
        }

        if blocks.is_empty() {
            String::new()
        } else {
            format!("{}\n", blocks.join("\n\n"))
        }
    }

    /// The document as the summary writes it.
    ///
    /// The title, the versions and the categories are the details'; each entry is a numbered list
    /// item whose title links to the paragraph the entry holds in the details at `path`.
    #[must_use]
    pub fn render_summary(&self, path: &str) -> String {
        let mut blocks = Vec::new();

        let before = self.before.join("\n");
        if !before.trim().is_empty() {
            blocks.push(before.trim().to_owned());
        }

        let mut slugs = Slugs::default();

        for version in &self.versions {
            let mut block = version.heading();
            let body =
                VersionBody::parse(&version.body, version.level).render_summary(path, &mut slugs);

            if !body.trim().is_empty() {
                block.push_str("\n\n");
                block.push_str(body.trim());
            }

            blocks.push(block);
        }

        if blocks.is_empty() {
            String::new()
        } else {
            format!("{}\n", blocks.join("\n\n"))
        }
    }

    /// Adds `entry` under `category` to `version`, opening the version when it is not there.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Archived`] when the version is sealed, and [`Error::StaleUnreleased`] when
    /// the changelog is still open for another version.
    pub fn add_entry(
        &mut self,
        version: &str,
        category: Category,
        entry: Entry,
    ) -> Result<(), Error> {
        let index = self.ensure_unreleased(version)?;
        let level = self.versions[index].level;
        let mut body = VersionBody::parse(&self.versions[index].body, level);

        body.add_entry(category, entry);
        body.sort();

        self.versions[index].body = body.render().lines().map(str::to_owned).collect();

        Ok(())
    }

    /// Seals the newest version still marked `(Unreleased)` on `date`, and answers its version.
    ///
    /// # Errors
    ///
    /// Reports [`Error::NoUnreleased`] when there is no such version.
    pub fn archive_newest(&mut self, date: &str) -> Result<String, Error> {
        let index = self
            .versions
            .iter()
            .position(|version| matches!(&version.status, Status::Unreleased))
            .ok_or(Error::NoUnreleased)?;

        let version = self.versions[index].version.clone();
        let level = self.versions[index].level;
        let mut body = VersionBody::parse(&self.versions[index].body, level);

        body.sort();
        self.versions[index].body = body.render().lines().map(str::to_owned).collect();
        self.versions[index].status = Status::Released(date.to_owned());

        Ok(version)
    }

    /// The index of `version`'s open section, opening one when there is none.
    fn ensure_unreleased(&mut self, version: &str) -> Result<usize, Error> {
        if let Some(index) = self
            .versions
            .iter()
            .position(|current| current.version == version)
        {
            return match &self.versions[index].status {
                Status::Unreleased => Ok(index),
                Status::Released(_) => Err(Error::Archived {
                    version: version.to_owned(),
                }),
            };
        }

        if let Some(stale) = self
            .versions
            .iter()
            .find(|current| matches!(&current.status, Status::Unreleased))
        {
            return Err(Error::StaleUnreleased {
                stale: stale.version.clone(),
                current: version.to_owned(),
            });
        }

        let level = self.versions.first().map_or(1, |current| current.level);
        self.versions.insert(
            0,
            Version {
                level,
                version: version.to_owned(),
                status: Status::Unreleased,
                body: Vec::new(),
            },
        );

        Ok(0)
    }
}

/// The summary of `details`, with each entry linking to the details at `path`.
///
/// The details are read back from their text rather than taken from an in-memory document, so the
/// numbers and anchors a summary names are the ones the file actually spells.
#[must_use]
pub fn summary(details: &str, path: &str) -> String {
    Document::parse(details).render_summary(path)
}

/// One version's body, split into the categories it recognizes and the text it does not.
#[derive(Clone, Debug)]
struct VersionBody {
    /// The heading level the version is written at.
    version_level: usize,
    /// The text before the first category.
    intro: Vec<String>,
    /// The categories, in the order they were read.
    categories: Vec<CategorySection>,
    /// The text after the last category.
    outro: Vec<String>,
}

impl VersionBody {
    /// Reads `lines` as the body of a version written at `version_level`.
    fn parse(lines: &[String], version_level: usize) -> Self {
        let mut body = Self {
            version_level,
            intro: Vec::new(),
            categories: Vec::new(),
            outro: Vec::new(),
        };
        let mut index = 0;

        while index < lines.len() {
            if let Some((level, text)) = heading(&lines[index])
                && level > version_level
                && Category::from_heading(text).is_some()
            {
                break;
            }

            body.intro.push(lines[index].clone());
            index += 1;
        }

        if index == lines.len() {
            return body;
        }

        let category_level = heading(&lines[index]).map_or(version_level + 1, |(level, _)| level);

        while index < lines.len() {
            let Some((level, text)) = heading(&lines[index]) else {
                body.outro.push(lines[index].clone());
                index += 1;
                continue;
            };

            let Some(category) = Category::from_heading(text) else {
                body.outro.push(lines[index].clone());
                index += 1;
                continue;
            };

            if level != category_level {
                body.outro.push(lines[index].clone());
                index += 1;
                continue;
            }

            let start = index;
            index += 1;

            while index < lines.len() {
                if let Some((next_level, next_text)) = heading(&lines[index])
                    && next_level == category_level
                    && Category::from_heading(next_text).is_some()
                {
                    break;
                }

                index += 1;
            }

            body.categories
                .push(parse_category(&lines[start..index], category, level));
        }

        body
    }

    /// Adds `entry` under `category`, opening the category when it is not there.
    fn add_entry(&mut self, category: Category, entry: Entry) {
        let index = if let Some(index) = self
            .categories
            .iter()
            .position(|section| section.category == category)
        {
            index
        } else {
            let level = self
                .categories
                .first()
                .map_or(self.version_level + 1, |section| section.level);

            self.categories.push(CategorySection {
                category,
                level,
                intro: Vec::new(),
                entries: Vec::new(),
                outro: Vec::new(),
            });

            self.categories.len() - 1
        };

        self.categories[index].add_entry(entry);
        self.categories.sort_by_key(|section| section.category);
    }

    /// Puts the categories and their entries in the order a version writes.
    fn sort(&mut self) {
        self.categories.sort_by_key(|section| section.category);

        for section in &mut self.categories {
            section.sort_entries();
        }
    }

    /// The body as it is written.
    fn render(&self) -> String {
        let mut blocks = Vec::new();

        let intro = self.intro.join("\n");
        if !intro.trim().is_empty() {
            blocks.push(intro.trim().to_owned());
        }

        let mut categories = self.categories.clone();
        categories.sort_by_key(|section| section.category);

        for section in &categories {
            let intro = section.intro.join("\n");
            let outro = section.outro.join("\n");

            if section.entries.is_empty() && intro.trim().is_empty() && outro.trim().is_empty() {
                continue;
            }

            blocks.push(section.render());
        }

        let outro = self.outro.join("\n");
        if !outro.trim().is_empty() {
            blocks.push(outro.trim().to_owned());
        }

        blocks.join("\n\n")
    }

    /// The body as the summary writes it: the same intro, categories and outro, with each entry a
    /// link instead of a paragraph.
    fn render_summary(&self, path: &str, slugs: &mut Slugs) -> String {
        let mut blocks = Vec::new();

        let intro = self.intro.join("\n");
        if !intro.trim().is_empty() {
            blocks.push(intro.trim().to_owned());
        }

        let mut categories: Vec<&CategorySection> = self.categories.iter().collect();
        categories.sort_by_key(|section| section.category);

        for section in categories {
            let intro = section.intro.join("\n");
            let outro = section.outro.join("\n");

            if section.entries.is_empty() && intro.trim().is_empty() && outro.trim().is_empty() {
                continue;
            }

            blocks.push(section.render_summary(path, slugs));
        }

        let outro = self.outro.join("\n");
        if !outro.trim().is_empty() {
            blocks.push(outro.trim().to_owned());
        }

        blocks.join("\n\n")
    }
}

/// One category under a version.
#[derive(Clone, Debug)]
struct CategorySection {
    /// The category this section is.
    category: Category,
    /// The heading level the category is written at.
    level: usize,
    /// The text between the heading and the first entry.
    intro: Vec<String>,
    /// The entries, in the order they were read.
    entries: Vec<Entry>,
    /// The text after the last entry.
    outro: Vec<String>,
}

impl CategorySection {
    /// Adds `entry`, at the level the section's entries are written at.
    fn add_entry(&mut self, entry: Entry) {
        let entry_level = self
            .entries
            .first()
            .map_or(self.level + 1, |entry| entry.heading_level);
        let mut entry = entry;

        entry.heading_level = entry_level;
        self.entries.push(entry);
        self.sort_entries();
    }

    /// Puts the entries in the order a category writes.
    fn sort_entries(&mut self) {
        self.entries.sort_by(Entry::compare);
    }

    /// The section as it is written.
    fn render(&self) -> String {
        let mut blocks = vec![format!(
            "{} {}",
            "#".repeat(self.level),
            self.category.heading()
        )];

        let intro = self.intro.join("\n");
        if !intro.trim().is_empty() {
            blocks.push(intro.trim().to_owned());
        }

        for (index, entry) in self.entries.iter().enumerate() {
            blocks.push(entry.render(index + 1));
        }

        let outro = self.outro.join("\n");
        if !outro.trim().is_empty() {
            blocks.push(outro.trim().to_owned());
        }

        blocks.join("\n\n")
    }

    /// The section as the summary writes it: its entries as a numbered list, each linking to the
    /// paragraph the details hold.
    fn render_summary(&self, path: &str, slugs: &mut Slugs) -> String {
        let mut blocks = vec![format!(
            "{} {}",
            "#".repeat(self.level),
            self.category.heading()
        )];

        let intro = self.intro.join("\n");
        if !intro.trim().is_empty() {
            blocks.push(intro.trim().to_owned());
        }

        if !self.entries.is_empty() {
            let mut list = String::new();

            for entry in &self.entries {
                let anchor = slugs.take(entry.anchor());

                writeln!(
                    list,
                    "{}. {} — [See details]({path}#{anchor})",
                    entry.number,
                    entry.title()
                )
                .ok();
            }

            blocks.push(list.trim_end().to_owned());
        }

        let outro = self.outro.join("\n");
        if !outro.trim().is_empty() {
            blocks.push(outro.trim().to_owned());
        }

        blocks.join("\n\n")
    }
}

/// The anchors a summary has handed out.
///
/// GitHub suffixes the second heading with the same anchor with `-1`, the third with `-2`, and so
/// on. The same is done here so a link lands on the heading it names even when two entries are
/// titled alike.
#[derive(Debug, Default)]
struct Slugs {
    /// How many times each base anchor has been written.
    seen: HashMap<String, usize>,
}

impl Slugs {
    /// The anchor for `base`, suffixed when it is not the first of its name.
    fn take(&mut self, base: String) -> String {
        let seen = self.seen.entry(base.clone()).or_insert(0);
        let anchor = if *seen == 0 {
            base
        } else {
            format!("{base}-{seen}")
        };

        *seen += 1;

        anchor
    }
}

/// The anchor GitHub gives a heading with `text`.
///
/// GitHub lowercases the heading, drops everything that is not a letter, a number, `_`, `-` or a
/// space, and turns each space into a hyphen. The rule is reproduced rather than approximated so a
/// summary's link lands on the heading it was written from.
fn slug(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());

    for character in text.chars() {
        if character.is_alphanumeric() || matches!(character, ' ' | '-' | '_') {
            slug.extend(character.to_lowercase());
        }
    }

    slug.replace(' ', "-")
}

/// The category `segment` is, read from its heading and the lines under it.
fn parse_category(segment: &[String], category: Category, level: usize) -> CategorySection {
    let entry_level = segment
        .iter()
        .filter_map(|line| {
            let (heading_level, text) = heading(line)?;

            (heading_level > level && Entry::from_heading(text, heading_level).is_some())
                .then_some(heading_level)
        })
        .min()
        .unwrap_or(level + 1);

    let mut section = CategorySection {
        category,
        level,
        intro: Vec::new(),
        entries: Vec::new(),
        outro: Vec::new(),
    };
    let mut current: Option<Entry> = None;
    let mut stopped = false;

    for line in &segment[1..] {
        if !stopped && let Some((heading_level, text)) = heading(line) {
            if heading_level == entry_level {
                if let Some(entry) = Entry::from_heading(text, heading_level) {
                    if let Some(finished) = current.take() {
                        section.entries.push(finished);
                    }

                    current = Some(entry);
                    continue;
                }

                stopped = true;
            } else if heading_level <= level {
                stopped = true;
            }
        }

        if stopped {
            if let Some(finished) = current.take() {
                section.entries.push(finished);
            }

            section.outro.push(line.clone());
        } else if let Some(entry) = current.as_mut() {
            entry.body.push(line.clone());
        } else {
            section.intro.push(line.clone());
        }
    }

    if let Some(finished) = current.take() {
        section.entries.push(finished);
    }

    section
}

/// A heading line, as its level and its text.
fn heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim();
    let level = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();

    if level == 0 {
        return None;
    }

    let text = trimmed[level..].strip_prefix(' ')?.trim();
    if text.is_empty() {
        return None;
    }

    Some((level, text))
}

/// A version heading, as its level, its version and its status.
fn version_heading(line: &str) -> Option<(usize, String, Status)> {
    let (level, text) = heading(line)?;
    let close = text.rfind(')')?;

    if close + 1 != text.len() {
        return None;
    }

    let open = text[..close].rfind('(')?;
    let version = text[..open].trim();
    if version.is_empty() {
        return None;
    }

    let status = &text[open + 1..close];
    let status = if status.eq_ignore_ascii_case("unreleased") {
        Status::Unreleased
    } else if is_iso_date(status) {
        Status::Released(status.to_owned())
    } else {
        return None;
    };

    Some((level, version.to_owned(), status))
}

/// Whether `text` is a date as `archive` writes it.
fn is_iso_date(text: &str) -> bool {
    if !text.is_ascii() || text.len() != 10 {
        return false;
    }

    let bytes = text.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }

    if !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..].iter().all(u8::is_ascii_digit)
    {
        return false;
    }

    let month: u32 = text[5..7].parse().unwrap_or(0);
    let day: u32 = text[8..].parse().unwrap_or(0);

    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// `rest` without a breaking marker, and whether one was there.
fn strip_breaking(rest: &str) -> (bool, &str) {
    for marker in [
        "**\\[BREAKING!\\]**",
        "**[BREAKING!]**",
        "\\[BREAKING!\\]",
        "[BREAKING!]",
    ] {
        if let Some(after) = rest.strip_prefix(marker) {
            return (true, after.trim_start());
        }
    }

    (false, rest)
}

/// The scopes a title starts with, and the text after them.
///
/// A scope is written as `` [`name`] ``. Only the backticked form is read, so a summary that starts
/// with an ordinary bracket — a link, for instance — is left as the summary.
fn parse_scopes(mut rest: &str) -> (Vec<String>, &str) {
    let mut scopes = Vec::new();

    loop {
        let trimmed = rest.trim_start();
        let Some(after_open) = trimmed.strip_prefix("[`") else {
            break;
        };
        let Some(end) = after_open.find("`]") else {
            break;
        };

        let scope = &after_open[..end];
        if scope.is_empty()
            || scope
                .chars()
                .any(|character| matches!(character, '[' | ']' | '`') || character.is_whitespace())
        {
            break;
        }

        scopes.push(scope.to_owned());
        rest = &after_open[end + 2..];
    }

    (scopes, rest)
}

/// The order two scope hierarchies are written in.
///
/// Each level is compared as a word, case-insensitively, and the first level that differs decides.
/// A hierarchy that is the beginning of another comes first.
fn compare_scopes(left: &[String], right: &[String]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let order = left.to_lowercase().cmp(&right.to_lowercase());
        if order != Ordering::Equal {
            return order;
        }

        let order = left.cmp(right);
        if order != Ordering::Equal {
            return order;
        }
    }

    left.len().cmp(&right.len())
}

#[cfg(test)]
mod tests {
    use super::{Category, Document, Entry};

    /// An entry with `body` written as lines.
    fn entry(breaking: bool, scopes: &[&str], summary: &str, body: &str) -> Entry {
        Entry {
            breaking,
            scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
            summary: summary.to_owned(),
            body: body.lines().map(str::to_owned).collect(),
            heading_level: 0,
            number: 0,
        }
    }

    #[test]
    fn an_entry_opens_a_version_and_a_category() {
        let mut document = Document::parse("");
        document
            .add_entry(
                "0.1.0",
                Category::Bugfix,
                entry(false, &[], "Summary", "Content."),
            )
            .unwrap();

        assert_eq!(
            document.render(),
            "# 0.1.0 (Unreleased)\n\n## Bugfix\n\n### 1. Summary\n\nContent.\n"
        );
    }

    #[test]
    fn a_version_writes_only_the_categories_it_has_entries_for() {
        let mut document = Document::parse("");

        document
            .add_entry("0.1.0", Category::Bugfix, entry(false, &[], "Fixed", ""))
            .unwrap();

        let rendered = document.render();
        assert!(rendered.contains("## Bugfix"));
        assert!(!rendered.contains("## Optimized"));
        assert!(!rendered.contains("## Feature"));
        assert!(!rendered.contains("## Refactor"));
    }

    #[test]
    fn breaking_entries_come_first_then_scopes_then_summaries() {
        let text = "\
# 0.1.0 (Unreleased)

## Bugfix

### 1. [`zeta`] Later

### 2. [`alpha`] First

### 3. **\\[BREAKING!\\]** [`omega`] Breaking
";
        let mut document = Document::parse(text);
        document.archive_newest("2026-10-25").unwrap();

        assert_eq!(
            document.render(),
            "\
# 0.1.0 (2026-10-25)

## Bugfix

### 1. **\\[BREAKING!\\]** [`omega`] Breaking

### 2. [`alpha`] First

### 3. [`zeta`] Later
"
        );
    }

    #[test]
    fn scopes_are_compared_level_by_level() {
        let mut entries = [
            entry(false, &["beta"], "B", ""),
            entry(false, &["alpha", "z"], "AZ", ""),
            entry(false, &["alpha", "a"], "AA", ""),
            entry(true, &["zeta"], "Breaking", ""),
        ];
        entries.sort_by(Entry::compare);

        let summaries: Vec<&str> = entries.iter().map(|entry| entry.summary.as_str()).collect();
        assert_eq!(summaries, ["Breaking", "AA", "AZ", "B"]);
    }

    #[test]
    fn categories_are_written_in_the_order_the_readme_states() {
        let text = "\
# 0.1.0 (Unreleased)

## Feature

### 1. F

## Bugfix

### 1. B
";
        let mut document = Document::parse(text);
        document.archive_newest("2026-10-25").unwrap();

        let rendered = document.render();
        let bugfix = rendered.find("## Bugfix").unwrap();
        let feature = rendered.find("## Feature").unwrap();

        assert!(bugfix < feature, "{rendered}");
    }

    #[test]
    fn a_sealed_version_cannot_be_written_to() {
        let mut document = Document::parse("# 0.1.0 (Unreleased)\n");
        document.archive_newest("2026-10-25").unwrap();

        let refused = document.add_entry("0.1.0", Category::Bugfix, entry(false, &[], "S", ""));

        assert!(matches!(
            refused,
            Err(super::Error::Archived { version }) if version == "0.1.0"
        ));
    }

    #[test]
    fn a_version_the_workspace_left_behind_is_refused() {
        let mut document = Document::parse("# 0.1.0 (Unreleased)\n");

        let refused = document.add_entry("0.2.0", Category::Bugfix, entry(false, &[], "S", ""));

        assert!(matches!(
            refused,
            Err(super::Error::StaleUnreleased { stale, current })
                if stale == "0.1.0" && current == "0.2.0"
        ));
    }

    #[test]
    fn archiving_seals_the_newest_open_version_and_leaves_the_others_alone() {
        let text = "\
# 0.2.0 (Unreleased)

## Feature

### 1. New

# 0.1.0 (2025-01-01)

## Bugfix

### 1. Old
";
        let mut document = Document::parse(text);
        let archived = document.archive_newest("2026-10-25").unwrap();

        assert_eq!(archived, "0.2.0");

        let rendered = document.render();
        assert!(rendered.contains("# 0.2.0 (2026-10-25)"));
        assert!(rendered.contains("# 0.1.0 (2025-01-01)"));
        assert!(rendered.contains("### 1. Old"));
    }

    #[test]
    fn there_is_nothing_to_archive_when_no_version_is_open() {
        let mut document = Document::parse("# 0.1.0 (2025-01-01)\n");

        assert!(matches!(
            document.archive_newest("2026-10-25"),
            Err(super::Error::NoUnreleased)
        ));
    }

    #[test]
    fn an_entrys_body_is_carried_through_a_rewrite() {
        let text = "\
# 0.1.0 (Unreleased)

## Bugfix

### 1. Summary

A body.

#### A deeper heading

More.
";
        let mut document = Document::parse(text);
        document.archive_newest("2026-10-25").unwrap();

        let rendered = document.render();
        assert!(rendered.contains("A body."));
        assert!(rendered.contains("#### A deeper heading"));
        assert!(rendered.contains("More."));
    }

    #[test]
    fn an_entry_title_is_read_back() {
        let entry =
            Entry::from_heading("1. **\\[BREAKING!\\]** [`cli`] [`vault`] Summary", 3).unwrap();

        assert!(entry.breaking);
        assert_eq!(entry.scopes.join(","), "cli,vault");
        assert_eq!(entry.summary, "Summary");
        assert_eq!(entry.heading_level, 3);
    }

    #[test]
    fn a_title_that_is_not_numbered_is_not_an_entry() {
        assert!(Entry::from_heading("Summary", 3).is_none());
        assert!(Entry::from_heading("1.5 Things", 3).is_none());
        assert!(Entry::from_heading("2026-10-25", 3).is_none());
    }

    #[test]
    fn a_title_writes_the_breaking_marker_before_the_scopes() {
        let mut entry = entry(true, &["cli", "rola"], "Summary", "");
        entry.heading_level = 3;

        assert_eq!(
            entry.render(2),
            "### 2. **\\[BREAKING!\\]** [`cli`] [`rola`] Summary"
        );
    }

    #[test]
    fn a_document_with_a_title_keeps_it() {
        let text = "# Changelog\n\nAll notable changes.\n\n# 0.1.0 (Unreleased)\n";
        let mut document = Document::parse(text);
        document
            .add_entry("0.1.0", Category::Bugfix, entry(false, &[], "S", ""))
            .unwrap();

        assert!(
            document
                .render()
                .starts_with("# Changelog\n\nAll notable changes.")
        );
    }

    #[test]
    fn a_summary_links_each_entry_to_its_paragraph_in_the_details() {
        let text = "\
# Changelog

All notable changes.

# 0.1.0 (Unreleased)

## Bugfix

### 1. **\\[BREAKING!\\]** [`cli`] [`rola`] Fix it

The body.

### 2. [`vault`] Summary

More.
";

        assert_eq!(
            super::summary(text, "dev/CHANGELOG-DETAILS.md"),
            "\
# Changelog

All notable changes.

# 0.1.0 (Unreleased)

## Bugfix

1. **\\[BREAKING!\\]** [`cli`] [`rola`] Fix it — [See details](dev/CHANGELOG-DETAILS.md#1-breaking-cli-rola-fix-it)
2. [`vault`] Summary — [See details](dev/CHANGELOG-DETAILS.md#2-vault-summary)
"
        );
    }

    #[test]
    fn a_summary_leaves_out_the_bodies() {
        let text = "# 0.1.0 (Unreleased)\n\n## Bugfix\n\n### 1. Summary\n\nA body.\n";
        let summary = super::summary(text, "details.md");

        assert!(summary.contains("1. Summary — [See details](details.md#1-summary)"));
        assert!(!summary.contains("A body."));
    }

    #[test]
    fn a_repeated_heading_gets_the_suffix_github_gives_it() {
        let text = "\
# 0.1.0 (Unreleased)

## Bugfix

### 1. Same

## Feature

### 1. Same
";
        let summary = super::summary(text, "details.md");

        assert!(summary.contains("details.md#1-same)"));
        assert!(summary.contains("details.md#1-same-1)"));
    }

    #[test]
    fn an_anchor_drops_punctuation_and_lowercases() {
        assert_eq!(
            super::slug("1. **\\[BREAKING!\\]** [`app`] It's [done]."),
            "1-breaking-app-its-done"
        );
    }
}
