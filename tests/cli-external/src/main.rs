//! `cli-external`: a word that names no command is handed to the program beside `rola`.
//!
//! `rola daemon` is `rola-daemon`: the two are exported together, and what a run writes after `rola`
//! may be the rest of another program's name. What is checked is what a caller meets — the program
//! is run, its output is the terminal's, and the run ends with its code — which is why this suite
//! lays the programs out itself and runs the copy of `rola` it made, rather than whatever happens to
//! sit beside the built one.
//!
//! The companion is `rola-daemon`, copied under names of its own: the hand-over is not a rule about
//! the word `daemon`, and a name with hyphens in it is reached as one word. Its `listen --help` is
//! asked for because it is an answer nothing else gives — `rola`'s own help does not name
//! `vault.toml` — so what comes back says which program answered.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rorolala_utils_sandbox::{Guard, bin_dir, run};

/// The word the companion is reached by, which is what follows `rola-`.
const WORD: &str = "probe";

/// The word of a name with hyphens in it, which is one word and not three.
const HYPHENS: &str = "aa-bb-cc";

/// What only the daemon's own help says, in either language it is written in.
const ITS_OWN_WORDS: &str = "vault.toml";

fn main() {
    let sandbox = Guard::new("cli-external");
    let here = sandbox.join("bin");
    fs::create_dir_all(&here).expect("the directory the programs are laid out in");

    let rola = lay(&here, "rola", "rola");
    lay(&here, "rola-daemon", &format!("rola-{WORD}"));
    lay(&here, "rola-daemon", &format!("rola-{HYPHENS}"));

    // A word naming the program beside this one, with the program's own arguments after it: the word
    // is the name and not an argument, so what the program is asked about is `listen`.
    for word in [WORD, HYPHENS] {
        let handed = run(Command::new(&rola).args([word, "listen", "--help"]));

        assert!(
            handed.stderr.contains(ITS_OWN_WORDS),
            "`{word}`: the daemon's own help was expected, got:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            handed.stdout,
            handed.stderr
        );
        assert!(
            handed.stdout.is_empty(),
            "`{word}`: nothing was expected on standard output, got:\n{}",
            handed.stdout
        );

        // The code is the program's own. `listen --help` is help, which is 2 where the daemon is
        // concerned — and the same 2 where this program is — so what says which program answered is
        // the words above and not this number.
        assert_eq!(
            Some(2),
            handed.code,
            "`{word}`: the daemon's help code was expected, got {:?}\n--- stderr ---\n{}",
            handed.code,
            handed.stderr
        );
    }

    // Three words are three words: the name is what follows `rola-`, so `aa bb cc` names no program
    // and is the mistake it always was.
    let apart = run(Command::new(&rola).args(HYPHENS.split('-')));
    assert_eq!(Some(12), apart.code);
    assert!(
        apart.stderr.contains("aa bb cc"),
        "the words as they were typed were expected, got:\n{}",
        apart.stderr
    );

    // A word naming nothing is this program's own mistake, answered the way it always was.
    let unknown = run(Command::new(&rola).arg("nosuch"));
    assert_eq!(Some(12), unknown.code);
    assert!(
        unknown.stderr.contains("nosuch"),
        "the unknown word was expected to be named, got:\n{}",
        unknown.stderr
    );
}

/// Copies a built program beside the copied `rola`, under the name a word reaches it by.
///
/// The name is put together rather than written out once per platform, as the program does it: a
/// platform names a program with a suffix or without, so what a word reaches is
/// `rola-<word><suffix>`.
fn lay(directory: &Path, program: &str, name: &str) -> PathBuf {
    let suffix = std::env::consts::EXE_SUFFIX;
    let from = bin_dir().join(format!("{program}{suffix}"));
    let to = directory.join(format!("{name}{suffix}"));

    fs::copy(&from, &to).unwrap_or_else(|error| panic!("copying {}: {error}", from.display()));

    to
}
