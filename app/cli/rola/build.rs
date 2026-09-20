//! Build script: turn the exit codes' own keys into a way to read a code back.
//!
//! `src/exit_codes.rs` states every exit code the program has, and each one carries the
//! `i18n/exit_codes.yml` key that says what it means, written above it as a doc comment.
//! This script reads those keys and generates `src/exit_codes/explain.rs`: the list of
//! codes there are, and `explain_ec`, which looks a code up and speaks the words the key
//! holds in the run's language.
//!
//! The file is shaped by `tmpl/explain.tmpl`, the way the daemon's are shaped by theirs: the
//! prose that says what the module is lives in the template, and what is here is only the
//! reading and the filling in. A template that cannot expand — its blocks do not pair up —
//! stops the build rather than writing out a file that is only half generated.
//!
//! The keys are read as text rather than with `syn`, unlike the daemon's build script. A key
//! belongs to a constant here, and a constant is one line; reading the line above it needs
//! nothing more than that, and what is written out is a `match` arm calling `t!`, so the
//! words themselves stay where a translation belongs.
//!
//! A code with no key above it stops the build. `explain_ec` answers an empty string to a
//! code it does not state, and a code whose key is missing would be one the program states
//! and cannot speak — which is the one thing this module exists to prevent.
//!
//! The file is written into the source tree, as `src/exit_codes/explain.rs`, and not into
//! `OUT_DIR`, following `modules/daemon`'s build script: what comes out is ordinary source,
//! and putting it where a reader can open it is worth the one rule that brings — the file is
//! generated, so it is not edited.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use just_template::Template;

/// The file the exit codes are read from, relative to the manifest directory.
const SOURCE: &str = "src/exit_codes.rs";

/// The template the module is rendered from, relative to the manifest directory.
const TEMPLATE: &str = "tmpl/explain.tmpl";

/// The file the module is written to, relative to the manifest directory.
const OUTPUT: &str = "src/exit_codes/explain.rs";

/// The directory the module is written into, relative to the manifest directory.
const OUTPUT_DIR: &str = "src/exit_codes";

/// The template's area for the codes, repeated once per line of the array.
const CODES_BLOCK: &str = "codes";

/// The template's area for the `match` arms, repeated once per code.
const EXPLAINS_BLOCK: &str = "explains";

/// The prefix a constant is written with.
const CONSTANT_PREFIX: &str = "pub const ";

/// What separates a constant's name from its type.
const NAME_SEPARATOR: &str = ": i32 =";

/// The prefix a key is written with.
const KEY_PREFIX: &str = "///";

fn main() {
    println!("cargo:rerun-if-changed={SOURCE}");
    println!("cargo:rerun-if-changed={TEMPLATE}");
    // The directory the module is written into is watched rather than the module itself:
    // a directory's timestamp moves when a file is added to it or removed from it, which is
    // what makes a deleted module be generated again, while rewriting one leaves it alone —
    // watching the module would be watching this script's own output, and the build would
    // never settle.
    println!("cargo:rerun-if-changed={}", OUTPUT_DIR);
    println!("cargo:rerun-if-changed=build.rs");

    let manifest = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR for build scripts"),
    );
    let source = manifest.join(SOURCE);

    let text = fs::read_to_string(&source).unwrap_or_else(|error| {
        fail(&format!("reading {}: {error}", source.display()));
    });

    let codes = stated_codes(&text).unwrap_or_else(|error| fail(&error));
    if codes.is_empty() {
        fail(&format!(
            "{} states no exit code: every constant of the form `{CONSTANT_PREFIX}EC_NAME\
             {NAME_SEPARATOR} N;` is read, and one is wanted",
            source.display()
        ));
    }

    let output = manifest.join(OUTPUT);
    if let Some(directory) = output.parent()
        && let Err(error) = fs::create_dir_all(directory)
    {
        fail(&format!("creating {}: {error}", directory.display()));
    }

    write_if_changed(&output, &render(&manifest, &codes));
}

/// One exit code, as its source spells it: the number, and the key written above it.
struct ExitCode {
    /// The number the constant states.
    code: i32,
    /// The `i18n/exit_codes.yml` key that says what it means.
    key: String,
}

/// Every exit code `text` states, in the order they are written.
///
/// A key is held until something else is reached: a constant takes the key standing
/// directly above it as its own, and a blank line clears what is standing. That is what
/// leaves a section heading — one on a line of its own — out of the constant that follows
/// it, while a key is still read from the line it is written on.
///
/// The key is read with or without the backticks that `clippy::doc_markdown` wants around
/// it: the source writes it backticked, so that the comment reads as one, and the key is
/// what stands between them.
///
/// # Errors
///
/// Returns why, when a constant has no key above it, when a key is written over more than
/// one line, or when two constants state one number.
fn stated_codes(text: &str) -> Result<Vec<ExitCode>, String> {
    let mut codes: Vec<ExitCode> = Vec::new();
    let mut standing: Option<&str> = None;

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(key) = trimmed.strip_prefix(KEY_PREFIX) {
            let key = key.trim().trim_matches('`');
            if key.is_empty() {
                return Err("a key is written above an exit code and is empty".to_string());
            }

            if standing.is_some_and(|held| held != key) {
                return Err(format!(
                    "`{key}` and another key are written above one constant"
                ));
            }

            standing = Some(key);
            continue;
        }

        if trimmed.is_empty() {
            standing = None;
            continue;
        }

        if let Some((name, code)) = constant_of(trimmed) {
            let Some(key) = standing else {
                return Err(format!("`{name}` states no key above it"));
            };

            if let Some(seen) = codes.iter().find(|other| other.code == code) {
                return Err(format!("`{name}` and `{}` both state {code}", seen.key));
            }

            codes.push(ExitCode {
                code,
                key: key.to_string(),
            });
        }

        standing = None;
    }

    Ok(codes)
}

/// The name and number `line` states, when it is a `pub const NAME: i32 = N;` line.
fn constant_of(line: &str) -> Option<(&str, i32)> {
    let rest = line.strip_prefix(CONSTANT_PREFIX)?;
    let (name, rest) = rest.split_once(NAME_SEPARATOR)?;
    let value = rest.trim_end().strip_suffix(';')?;

    Some((name.trim(), value.trim().parse().ok()?))
}

/// The module the explanations are written as.
///
/// The codes are in number order rather than the order the source writes them in: the
/// source groups them by what they are about, and a reader who has a number wants it by
/// number.
fn render(manifest: &Path, codes: &[ExitCode]) -> String {
    let path = manifest.join(TEMPLATE);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => fail(&format!("reading {}: {error}", path.display())),
    };

    let mut sorted: Vec<&ExitCode> = codes.iter().collect();
    sorted.sort_by_key(|code| code.code);

    let mut template = Template::from(source);

    // Each area is fed one map per code, so the template decides where a code and its key
    // go: what is here is the pairing, and what a code is written as is the template's.
    {
        let lines = template.add_impl(CODES_BLOCK.to_owned());
        for code in &sorted {
            lines.push(HashMap::from([("code".to_owned(), code.code.to_string())]));
        }
    }

    {
        let arms = template.add_impl(EXPLAINS_BLOCK.to_owned());
        for code in &sorted {
            arms.push(HashMap::from([
                ("code".to_owned(), code.code.to_string()),
                ("key".to_owned(), code.key.clone()),
            ]));
        }
    }

    template.expand().map_or_else(
        || {
            fail(&format!(
                "expanding {}: its blocks do not pair up, or a block is nested",
                path.display()
            ))
        },
        // `expand` trims what it produces, so the newline a text file ends with is put back
        // here.
        |rendered| rendered + "\n",
    )
}

/// Writes `contents` to `path`, unless it already says exactly that.
///
/// Rewriting a file that has not changed would give it a new timestamp, and a fresh
/// timestamp is what makes a build script run again.
fn write_if_changed(path: &Path, contents: &str) {
    if fs::read_to_string(path).is_ok_and(|existing| existing == contents) {
        return;
    }

    if let Err(error) = fs::write(path, contents) {
        fail(&format!("writing {}: {error}", path.display()));
    }
}

/// Reports `message` to cargo and stops the build.
///
/// A build script can only speak to cargo through its directives, so the diagnostic is
/// forwarded line by line and the script then exits non-zero: a file that is only partly
/// generated must never be left behind as if it were whole.
fn fail(message: &str) -> ! {
    for line in message.lines() {
        println!("cargo:warning={line}");
    }

    exit(1);
}
