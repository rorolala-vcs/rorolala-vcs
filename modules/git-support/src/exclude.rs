//! The block of a repository's `info/exclude` that keeps a project's materials out of git.

use std::fs;
use std::io;
use std::path::Path;

use rola_define_format::Lock;

use crate::error::Error;

/// How the work tree names `project`, which is how its block is told apart from another project's.
///
/// `.` is the work tree's own root; anything else is the path from it, `/`-separated.
#[must_use]
pub fn name(project: &Path) -> String {
    let parts: Vec<String> = project
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();

    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

/// Rewrites `exclude` so that the project's materials are kept out of git, leaving every other
/// line as it was.
///
/// The block is the project's own: another project in the same work tree has one of its own, and a
/// person's own lines are not touched. Running it twice with the same lock writes the same bytes.
///
/// # Errors
///
/// Returns [`Error::Io`] when the file cannot be read or written.
pub fn rewrite(exclude: &Path, project: &str, lock: &Lock) -> Result<(), Error> {
    let begin = begin(project);
    let close = end(project);

    let (mut lines, line_ending) = read_lines(exclude)?;
    lines = without_block(lines, &begin, &close);

    let patterns = patterns(project, lock);
    if !patterns.is_empty() {
        lines.push(begin);
        lines.extend(patterns);
        lines.push(close);
    }

    let mut text = lines.join(line_ending);
    if !text.is_empty() {
        text.push_str(line_ending);
    }

    write_atomically(exclude, &text)
}

/// The line that opens a project's block.
fn begin(project: &str) -> String {
    format!("# >>> rola: {project} (managed; edits will be lost)")
}

/// The line that closes a project's block.
fn end(project: &str) -> String {
    format!("# <<< rola: {project}")
}

/// The patterns a project's block holds: the workspace's own data, then each material.
///
/// A pattern is the place as a work tree names it, anchored at the work tree's root, so the same
/// block means the same thing whatever directory git is run from.
fn patterns(project: &str, lock: &Lock) -> Vec<String> {
    let base = if project == "." {
        String::new()
    } else {
        format!("/{}", escape(project))
    };

    let mut patterns = Vec::with_capacity(lock.materials().len() + 1);
    patterns.push(format!("{base}/{}", escape(".rola") + "/"));

    for material in lock.materials() {
        patterns.push(format!("{base}/{}", escape(material.path().as_str())));
    }

    patterns
}

/// `path` as gitignore reads it literally.
///
/// What a pattern says beyond a place is `*`, `?` and `[`: a file named with one would be a rule
/// about other files, so each is escaped. A leading `!` or `#` is not escaped, because a pattern
/// always begins with `/` and neither means anything after it.
fn escape(path: &str) -> String {
    let mut escaped = String::with_capacity(path.len());

    for character in path.chars() {
        if matches!(character, '*' | '?' | '[' | ']' | '\\') {
            escaped.push('\\');
        }

        escaped.push(character);
    }

    escaped
}

/// The lines of `path`, and the line ending they were written with.
///
/// A file that does not exist reads as no lines at all: a repository that has never excluded
/// anything is the state a first run starts from.
fn read_lines(path: &Path) -> Result<(Vec<String>, &'static str), Error> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
            let lines = text.lines().map(str::to_owned).collect();

            Ok((lines, ending))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((Vec::new(), "\n")),
        Err(error) => Err(Error::Io(error)),
    }
}

/// `lines` without the block `begin` opens and `close` ends.
///
/// A block whose closing line is gone is taken to the end of the file: half a managed block is
/// this run's to remove, not to keep beside the one it writes.
fn without_block(lines: Vec<String>, begin: &str, close: &str) -> Vec<String> {
    let Some(start) = lines.iter().position(|line| line == begin) else {
        return lines;
    };

    let found = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.as_str() == close)
        .map(|(at, _)| at);

    let mut lines = lines;
    match found {
        Some(close) => {
            lines.drain(start..=close);
        }
        None => {
            lines.truncate(start);
        }
    }

    lines
}

/// Writes `text` to `path` whole, rather than leaving half a block in the file a reader is using.
fn write_atomically(path: &Path, text: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(Error::Io)?;
    }

    let temporary = temporary_path(path);
    fs::write(&temporary, text).map_err(Error::Io)?;

    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        // WORKAROUND: Windows refuses to rename over a file that is there, so the old file is
        // removed first. The path is the repository's own info directory, which no other program
        // writes while git is not being run.
        Err(error) if path.exists() => {
            fs::remove_file(path).map_err(Error::Io)?;
            fs::rename(&temporary, path).map_err(|_| Error::Io(error))
        }
        Err(error) => Err(Error::Io(error)),
    }
}

/// A path beside `path` this run may write to, which no other run's file collides with.
fn temporary_path(path: &Path) -> std::path::PathBuf {
    let name = path.file_name().map_or_else(
        || "exclude".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );

    path.with_file_name(format!(".{name}.rola-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use rola_define_format::{ContentHash, Lock, LockPath, Material};

    use super::{name, rewrite};
    use crate::error::Error;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-git-support-exclude-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A lock placing `paths`.
    fn lock(paths: &[&str]) -> Lock {
        let materials = paths
            .iter()
            .map(|path| {
                Material::new(
                    LockPath::new(path).unwrap(),
                    ContentHash::from_digest([7; 32]),
                )
            })
            .collect();

        Lock::new(vec![], materials).unwrap()
    }

    /// The text of `path`, as a test reads it.
    fn text(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn a_project_is_named_by_the_work_tree_as_it_lies() {
        assert_eq!(name(Path::new("")), ".");
        assert_eq!(name(Path::new(".")), ".");
        assert_eq!(name(Path::new("Sub/Project")), "Sub/Project");
    }

    #[test]
    fn a_first_run_writes_the_projects_block() {
        let dir = scratch("write");
        let exclude = dir.join("info").join("exclude");

        rewrite(&exclude, ".", &lock(&["/Assets/hero.psd", "/Source/a.fbx"])).unwrap();

        assert_eq!(
            text(&exclude),
            "# >>> rola: . (managed; edits will be lost)\n/.rola/\n/Assets/hero.psd\n/Source/a.fbx\n# <<< rola: .\n"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_run_with_the_same_lock_writes_the_same_bytes() {
        let dir = scratch("idempotent");
        let exclude = dir.join("info").join("exclude");
        let lock = lock(&["/Assets/hero.psd"]);

        rewrite(&exclude, ".", &lock).unwrap();
        let first = text(&exclude);
        rewrite(&exclude, ".", &lock).unwrap();

        assert_eq!(text(&exclude), first);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_persons_own_lines_are_left_as_they_were() {
        let dir = scratch("person");
        let exclude = dir.join("info").join("exclude");
        std::fs::create_dir_all(dir.join("info")).unwrap();
        std::fs::write(&exclude, "# mine\n*.tmp\n").unwrap();

        rewrite(&exclude, ".", &lock(&["/Assets/hero.psd"])).unwrap();

        assert_eq!(
            text(&exclude),
            "# mine\n*.tmp\n# >>> rola: . (managed; edits will be lost)\n/.rola/\n/Assets/hero.psd\n# <<< rola: .\n"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_block_is_replaced_rather_than_grown() {
        let dir = scratch("replace");
        let exclude = dir.join("info").join("exclude");

        rewrite(&exclude, ".", &lock(&["/Assets/hero.psd"])).unwrap();
        rewrite(&exclude, ".", &lock(&["/Source/a.fbx"])).unwrap();

        assert_eq!(
            text(&exclude),
            "# >>> rola: . (managed; edits will be lost)\n/.rola/\n/Source/a.fbx\n# <<< rola: .\n"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_projects_block_does_not_touch_anothers() {
        let dir = scratch("two");
        let exclude = dir.join("info").join("exclude");

        rewrite(&exclude, ".", &lock(&["/Assets/hero.psd"])).unwrap();
        rewrite(&exclude, "Sub", &lock(&["/Assets/bg.psd"])).unwrap();

        let written = text(&exclude);
        assert!(
            written.contains("# >>> rola: . (managed; edits will be lost)"),
            "{written}"
        );
        assert!(
            written.contains("# >>> rola: Sub (managed; edits will be lost)"),
            "{written}"
        );
        assert!(written.contains("/Sub/.rola/"), "{written}");
        assert!(written.contains("/Sub/Assets/bg.psd"), "{written}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pattern_escapes_what_gitignore_would_read_as_a_rule() {
        let dir = scratch("escape");
        let exclude = dir.join("info").join("exclude");

        rewrite(
            &exclude,
            ".",
            &lock(&["/Assets/star*name.psd", "/Assets/q?x.psd"]),
        )
        .unwrap();

        let written = text(&exclude);
        assert!(written.contains("/Assets/star\\*name.psd"), "{written}");
        assert!(written.contains("/Assets/q\\?x.psd"), "{written}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_carriage_return_file_keeps_its_line_endings() {
        let dir = scratch("crlf");
        let exclude = dir.join("info").join("exclude");
        std::fs::create_dir_all(dir.join("info")).unwrap();
        std::fs::write(&exclude, "# mine\r\n*.tmp\r\n").unwrap();

        rewrite(&exclude, ".", &lock(&["/Assets/hero.psd"])).unwrap();

        assert!(text(&exclude).contains("*.tmp\r\n# >>> rola:"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_half_written_block_is_taken_to_the_end_of_the_file() {
        let dir = scratch("half");
        let exclude = dir.join("info").join("exclude");
        std::fs::create_dir_all(dir.join("info")).unwrap();
        std::fs::write(
            &exclude,
            "# >>> rola: . (managed; edits will be lost)\n/old/left.psd\n",
        )
        .unwrap();

        rewrite(&exclude, ".", &lock(&["/Assets/hero.psd"])).unwrap();

        assert_eq!(
            text(&exclude),
            "# >>> rola: . (managed; edits will be lost)\n/.rola/\n/Assets/hero.psd\n# <<< rola: .\n"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_place_that_cannot_be_written_is_an_error() {
        let dir = scratch("blocked");
        // A directory where the file's own directory would go: nothing can be written there.
        let exclude = dir.join("info");
        std::fs::write(&exclude, b"not a directory").unwrap();

        // The file is the *parent* of nothing; asking for the file inside it fails.
        let result = rewrite(&dir.join("info").join("exclude"), ".", &lock(&[]));

        assert!(matches!(result, Err(Error::Io(_))), "{result:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
