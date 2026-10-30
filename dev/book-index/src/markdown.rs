//! The plain text an article is searched by, and the title a front matter block leaves out.
//!
//! The index is built from words, not from syntax. What is kept is what a reader would say the
//! article says: headings, paragraphs, list items, table cells and code are all text, and the
//! punctuation that says which of them it is is taken away. The result is one line, because a
//! fragment of it is shown beside a hit and a line break in a search result is noise.
//!
//! Markdown has more shapes than a search needs to know. What is handled here is the one the book
//! is written in: headings, lists, quotes, fenced code, tables, links and images, emphasis, inline
//! code and raw HTML. Anything else is left as it stands, which is what a search wants anyway.

/// The title an article falls back to when its front matter names none: its first heading.
///
/// The first heading is the one a reader would call the title, and a file whose front matter is
/// missing should still be found by what it says rather than by its file name alone.
#[must_use]
pub fn title(markdown: &str) -> Option<String> {
    let mut fence: Option<&str> = None;

    for line in markdown.lines() {
        let trimmed = line.trim();

        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }

        if let Some(marker) = fence_marker(trimmed) {
            fence = Some(marker);
            continue;
        }

        if let Some(heading) = heading(trimmed) {
            let heading = collapse(&inline(&block(heading)));

            if !heading.is_empty() {
                return Some(heading);
            }
        }
    }

    None
}

/// The plain text of a Markdown body.
///
/// Code is kept rather than dropped: a reader searching for a command expects the article that
/// shows it to be found, and what a fenced block holds is the text of the article as much as its
/// prose is. What is taken away is the fence itself and the syntax inside it, which is what a
/// search is not about.
#[must_use]
pub fn plain(markdown: &str) -> String {
    let mut blocks = Vec::new();
    let mut fence: Option<&str> = None;

    for line in markdown.lines() {
        let trimmed = line.trim();

        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            } else {
                // What a fenced block holds is code: its markers are part of what a search is
                // looking for, so the line is collapsed as it stands rather than read as Markdown.
                push(&mut blocks, trimmed);
            }
            continue;
        }

        if let Some(marker) = fence_marker(trimmed) {
            fence = Some(marker);
            continue;
        }

        if trimmed.is_empty() || is_rule(trimmed) || is_table_separator(trimmed) {
            continue;
        }

        push(&mut blocks, &inline(&block(trimmed)));
    }

    blocks.join(" ")
}

/// Pushes `text` onto the blocks when it says anything after whitespace is collapsed.
fn push(blocks: &mut Vec<String>, text: &str) {
    let text = collapse(text);

    if !text.is_empty() {
        blocks.push(text);
    }
}

/// The MARKER a fenced code block opens with, if the line opens one.
fn fence_marker(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("```") {
        Some("```")
    } else if trimmed.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

/// Whether a line is a horizontal rule rather than text.
fn is_rule(trimmed: &str) -> bool {
    let stripped: String = trimmed
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();

    stripped.len() >= 3
        && (stripped.chars().all(|character| character == '-')
            || stripped.chars().all(|character| character == '*')
            || stripped.chars().all(|character| character == '_'))
}

/// Whether a table line is the row of dashes under the header.
fn is_table_separator(trimmed: &str) -> bool {
    trimmed.starts_with('|')
        && trimmed.ends_with('|')
        && trimmed
            .chars()
            .all(|character| matches!(character, '|' | '-' | ':' | ' '))
        && trimmed.contains('-')
}

/// The heading's text, when the line is an ATX heading.
fn heading(trimmed: &str) -> Option<&str> {
    let hashes = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();

    if (1..=6).contains(&hashes)
        && let Some(text) = trimmed[hashes..].strip_prefix(' ')
    {
        return Some(text.trim_start());
    }

    None
}

/// The line with the block syntax taken off: quote, heading, list and task markers.
fn block(line: &str) -> String {
    let mut line = line.trim_start().to_owned();

    while let Some(rest) = line.strip_prefix('>') {
        line = rest.trim_start().to_owned();
    }

    if let Some(text) = heading(&line) {
        line = text.to_owned();
    }

    line = strip_list(&line);

    if let Some(rest) = line.strip_prefix("[ ] ") {
        line = rest.to_owned();
    } else if let Some(rest) = line.strip_prefix("[x] ") {
        line = rest.to_owned();
    } else if let Some(rest) = line.strip_prefix("[X] ") {
        line = rest.to_owned();
    }

    line
}

/// The line with a list marker taken off, when it begins with one.
fn strip_list(line: &str) -> String {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return rest.to_owned();
        }
    }

    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0
        && let Some(rest) = line.get(digits..)
        && let Some(rest) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))
    {
        return rest.to_owned();
    }

    line.to_owned()
}

/// The text with the inline syntax taken away and the bytes that are not text dropped.
fn inline(text: &str) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut at = 0;

    while at < characters.len() {
        let character = characters[at];

        match character {
            '`' | '*' | '_' | '~' => at += 1,
            '!' if characters.get(at + 1) == Some(&'[') => at += 1,
            '[' => {
                if let Some(close) = find(&characters, at + 1, ']') {
                    let label: String = characters[at + 1..close].iter().collect();
                    out.push_str(&inline(&label));
                    at = close + 1;

                    if characters.get(at) == Some(&'(')
                        && let Some(paren) = find(&characters, at + 1, ')')
                    {
                        at = paren + 1;
                    }

                    continue;
                }

                out.push(character);
                at += 1;
            }
            '<' => {
                // Only a tag is taken away: a comparison in prose (`a < b`) is text, and dropping
                // what stands between its brackets would be dropping an article's words.
                let opens_tag = characters
                    .get(at + 1)
                    .copied()
                    .is_some_and(|next| next.is_ascii_alphabetic() || next == '/' || next == '!');

                if opens_tag && let Some(close) = find(&characters, at + 1, '>') {
                    at = close + 1;
                } else {
                    out.push(character);
                    at += 1;
                }
            }
            '\\' => {
                if let Some(escaped) = characters.get(at + 1) {
                    out.push(*escaped);
                    at += 2;
                } else {
                    at += 1;
                }
            }
            '|' => {
                out.push(' ');
                at += 1;
            }
            _ => {
                out.push(character);
                at += 1;
            }
        }
    }

    out
}

/// Where `wanted` is in `characters`, from `from`.
fn find(characters: &[char], from: usize, wanted: char) -> Option<usize> {
    characters
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, character)| **character == wanted)
        .map(|(at, _)| at)
}

/// The text with its whitespace runs collapsed to single spaces.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{plain, title};

    #[test]
    fn the_first_heading_is_the_title_when_the_front_matter_names_none() {
        assert_eq!(
            title("Intro\n\n## How to **install**\n\nText"),
            Some("How to install".to_owned())
        );
    }

    #[test]
    fn a_heading_inside_a_code_block_is_not_the_title() {
        assert_eq!(
            title("```\n# Not this\n```\n\n# This"),
            Some("This".to_owned())
        );
    }

    #[test]
    fn block_syntax_is_taken_away_and_paragraphs_become_one_line() {
        assert_eq!(
            plain("# Title\n\n- one\n- two\n\n> quoted\n\n1. first"),
            "Title one two quoted first"
        );
    }

    #[test]
    fn links_keep_their_words_and_images_their_alt_text() {
        assert_eq!(
            plain("See [the guide](https://example.com) and ![a picture](a.png)."),
            "See the guide and a picture."
        );
    }

    #[test]
    fn emphasis_and_inline_code_lose_their_markers_and_keep_their_words() {
        assert_eq!(
            plain("A **bold** _word_ and `code`."),
            "A bold word and code."
        );
    }

    #[test]
    fn a_fenced_block_keeps_its_code_and_loses_its_fence() {
        assert_eq!(plain("Run:\n\n```sh\nrola init\n```\n"), "Run: rola init");
        assert_eq!(
            plain("```\na * b ** c\n```"),
            "a * b ** c",
            "code keeps the markers that are part of it"
        );
    }

    #[test]
    fn a_comparison_is_text_and_only_a_tag_is_taken_away() {
        assert_eq!(plain("a < b and c > d"), "a < b and c > d");
        assert_eq!(plain("one <b>bold</b> two"), "one bold two");
    }

    #[test]
    fn table_cells_are_words_and_the_separator_is_not() {
        assert_eq!(
            plain("| Flag | Meaning |\n| --- | --- |\n| --vault | a vault |"),
            "Flag Meaning --vault a vault"
        );
    }

    #[test]
    fn whitespace_is_collapsed() {
        assert_eq!(plain("a\n\n\n   b\t c  "), "a b c");
    }
}
