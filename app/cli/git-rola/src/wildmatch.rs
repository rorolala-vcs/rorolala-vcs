//! git's wildcards in a pathspec: whether a name with `*`, `?` or `[]` matches a path.
//!
//! A pathspec is a shell wildcard, not a regular expression, and git reads four environment
//! variables into how it is read: `GIT_LITERAL_PATHSPECS` and `GIT_NOGLOB_PATHSPECS` make every
//! name literal, `GIT_GLOB_PATHSPECS` makes `*` stop at a `/`, and `GIT_ICASE_PATHSPECS` folds the
//! case. The default is git's: `*` crosses `/`, so `Assets/*` selects everything under `Assets`.
//!
//! The matching itself is `globset`'s, which reads one thing git does not: `{a,b}` is a choice to
//! `globset` and three literal characters to git. A name is only compiled when it holds `*`, `?` or
//! `[`, so a name of braces alone is compared as git compares it; the extension is reached only by
//! a name that has a wildcard as well, `Assets/*{a,b}`, and is recorded rather than parsed away.

use globset::{GlobBuilder, GlobMatcher};

/// How a run's environment asks for pathspecs to be read.
#[derive(Clone, Copy, Default)]
pub struct Options {
    /// Every name is taken literally, wildcards and all.
    literal: bool,
    /// `*` does not cross `/`, which is what `:(glob)` and `GIT_GLOB_PATHSPECS` mean.
    one_directory: bool,
    /// Matching folds the case of letters.
    case_insensitive: bool,
}

impl Options {
    /// How this run's environment asks for pathspecs to be read.
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var_os(name).is_some())
    }

    /// How `present` says pathspecs are to be read.
    ///
    /// A variable is read for being there rather than for what it holds, which is what git's
    /// `getenv` does: `GIT_LITERAL_PATHSPECS=0` still says literal. The two variables that turn
    /// wildcards off win over the one that narrows them, and an empty `LANGUAGE`-style value is a
    /// value here, since the question is only whether it was set.
    pub fn from_lookup(present: impl Fn(&str) -> bool) -> Self {
        let literal = present("GIT_LITERAL_PATHSPECS") || present("GIT_NOGLOB_PATHSPECS");

        Self {
            literal,
            one_directory: !literal && present("GIT_GLOB_PATHSPECS"),
            case_insensitive: present("GIT_ICASE_PATHSPECS"),
        }
    }
}

/// One name as the run wrote it, with its wildcard form when it has one.
pub struct Pattern {
    /// The name as it was written and as it is reported.
    text: String,
    /// How the run reads pathspecs.
    options: Options,
    /// The compiled wildcard, when the name has one and the run reads wildcards.
    matcher: Option<GlobMatcher>,
}

impl Pattern {
    /// `text` read under `options`.
    pub fn new(text: String, options: Options) -> Self {
        let matcher = (!options.literal && has_wildcard(&text))
            .then(|| compile(&text, options, options.one_directory))
            .flatten();

        Self {
            text,
            options,
            matcher,
        }
    }

    /// The name as it was written.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether the name was read as a wildcard at all.
    pub fn has_wildcard(&self) -> bool {
        self.matcher.is_some()
    }

    /// Whether the name matches `path` whole.
    pub fn matches(&self, path: &str) -> bool {
        self.matcher.as_ref().map_or_else(
            || self.same(self.text(), path),
            |matcher| matcher.is_match(path),
        )
    }

    /// Whether the name is `path` itself or a directory above it.
    ///
    /// A directory name selects everything beneath it, so `Assets` selects `Assets/hero.psd` and
    /// `Sou?ce` selects `Source/c.fbx`. A name that was read as a wildcard can match any of the
    /// directory parts of the path, since `*` may cross a `/`; one that was not is compared with
    /// the whole directory prefix.
    pub fn at_or_above(&self, path: &str) -> bool {
        if self.matches(path) {
            return true;
        }

        if !self.has_wildcard() {
            let mut head = self.text().to_owned();
            head.push('/');

            return path
                .get(..head.len())
                .is_some_and(|start| self.same(start, &head));
        }

        let mut end = 0;

        while let Some(at) = path[end..].find('/') {
            end += at;

            if self.matches(&path[..end]) {
                return true;
            }

            end += 1;
        }

        false
    }

    /// Whether the name is a path below `path`, which is a tree's own directory.
    ///
    /// Only asked of a tree, since a path inside a file material names nothing.
    pub fn below(&self, path: &str) -> bool {
        let mut head = path.to_owned();
        head.push('/');

        self.text()
            .get(..head.len())
            .is_some_and(|start| self.same(start, &head))
    }

    /// Whether `part`, one component of the name, matches `name`, one component of a path.
    ///
    /// This is how a name is compared with a path a component at a time, which is what tells a name
    /// inside a tree from one beside it.
    pub fn matches_component(&self, part: &str, name: &str) -> bool {
        if !has_wildcard(part) {
            return self.same(part, name);
        }

        compile(part, self.options, true).is_some_and(|matcher| matcher.is_match(name))
    }

    /// Whether `left` and `right` are the same text under the run's case rule.
    fn same(&self, left: &str, right: &str) -> bool {
        if self.options.case_insensitive {
            left.eq_ignore_ascii_case(right)
        } else {
            left == right
        }
    }
}

/// Whether `text` holds a wildcard at all.
pub fn has_wildcard(text: &str) -> bool {
    text.contains(['*', '?', '['])
}

/// `text` compiled, or nothing when it is not a pattern `globset` reads.
///
/// A name with an unclosed `[` is not a wildcard git reads either: what it would match is a path
/// spelled that way, so the caller falls back to comparing the name literally.
fn compile(text: &str, options: Options, one_directory: bool) -> Option<GlobMatcher> {
    GlobBuilder::new(text)
        .literal_separator(one_directory)
        .case_insensitive(options.case_insensitive)
        .backslash_escape(false)
        .build()
        .ok()
        .map(|glob| glob.compile_matcher())
}

#[cfg(test)]
mod tests {
    use super::{Options, Pattern, has_wildcard};

    /// A pattern read with no environment behind it.
    fn pattern(text: &str) -> Pattern {
        Pattern::new((*text).to_owned(), Options::default())
    }

    #[test]
    fn a_star_crosses_a_slash_by_default() {
        assert!(pattern("Assets/*").matches("Assets/a/b.psd"));
        assert!(pattern("Assets/*").matches("Assets/hero.psd"));
        assert!(!pattern("Assets/*").matches("Source/c.fbx"));
    }

    #[test]
    fn a_question_mark_and_a_class_match_one_byte() {
        assert!(pattern("Assets/?.psd").matches("Assets/a.psd"));
        assert!(!pattern("Assets/?.psd").matches("Assets/ab.psd"));
        assert!(pattern("Assets/[ab].psd").matches("Assets/b.psd"));
        assert!(!pattern("Assets/[ab].psd").matches("Assets/c.psd"));
    }

    #[test]
    fn a_name_without_a_wildcard_is_compared_as_it_is() {
        assert!(pattern("Assets/hero.psd").matches("Assets/hero.psd"));
        assert!(!pattern("Assets/hero.psd").matches("Assets/hero.psdx"));
        assert!(!has_wildcard("Assets/hero.psd"));
    }

    #[test]
    fn a_literal_environment_makes_a_star_a_character() {
        let options = Options::from_lookup(|name| name == "GIT_LITERAL_PATHSPECS");

        assert!(!Pattern::new("Assets/*".to_owned(), options).has_wildcard());
        assert!(!Pattern::new("Assets/*".to_owned(), options).matches("Assets/hero.psd"));
    }

    #[test]
    fn a_noglob_environment_is_literal_too() {
        let options = Options::from_lookup(|name| name == "GIT_NOGLOB_PATHSPECS");

        assert!(!Pattern::new("Assets/*".to_owned(), options).has_wildcard());
    }

    #[test]
    fn the_glob_environment_stops_a_star_at_a_slash() {
        let options = Options::from_lookup(|name| name == "GIT_GLOB_PATHSPECS");
        let pattern = Pattern::new("Assets/*".to_owned(), options);

        assert!(pattern.matches("Assets/hero.psd"));
        assert!(!pattern.matches("Assets/a/b.psd"));
    }

    #[test]
    fn the_icase_environment_folds_the_case() {
        let options = Options::from_lookup(|name| name == "GIT_ICASE_PATHSPECS");

        assert!(Pattern::new("assets/HERO.psd".to_owned(), options).matches("Assets/hero.psd"));
    }

    #[test]
    fn an_unclosed_class_is_read_as_the_name_it_is() {
        let pattern = pattern("Assets/[a.psd");

        assert!(!pattern.has_wildcard());
        assert!(pattern.matches("Assets/[a.psd"));
    }

    #[test]
    fn a_name_of_braces_is_the_name_git_reads() {
        // git has no `{a,b}`: the braces and the comma are characters, and a name holding only
        // those is compared literally here.
        let pattern = pattern("Assets/{hero,other}.psd");

        assert!(!pattern.has_wildcard());
        assert!(pattern.matches("Assets/{hero,other}.psd"));
    }

    #[test]
    fn a_branch_beside_a_wildcard_is_the_extension_globset_has() {
        // The one place `globset` reads more than git: a name with a wildcard this program
        // compiles has its braces read as a choice.
        assert!(pattern("Assets/*{a,b}").matches("Assets/xb"));
        assert!(pattern("Assets/*{a,b}").matches("Assets/xa"));
        assert!(!pattern("Assets/*{a,b}").matches("Assets/xc"));
    }
}
