//! `ffi-c`: the C ABI, compiled and run as C11 programs.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: what
//! a C caller does is compile against the generated header, link the built library, run, and read
//! what came back. Nothing Rust reaches across the boundary here — the checks are the ones a C
//! program can make, made in C — so what is checked is the ABI itself rather than the library under
//! it.
//!
//! The checks are split into modules, one directory beside this one per area of the surface, each
//! named `ffi-<area>` and holding a `test.c` that is a C11 program of its own. Every directory with
//! such a program is compiled and run, so what is answered here is one question per module — and
//! one more for the library and the header being there at all.
//!
//! The library is built here rather than waited for, since the release programs do not depend on the
//! `rorolala-ffi` crate — a header without the library beside it proves nothing.

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use rorolala_utils_sandbox::bin_dir;

/// The file a module holds its own C11 program in.
const MODULE_SOURCE: &str = "test.c";

fn main() {
    let bin = bin_dir();
    // The workspace root as seen from this suite: `tests/ffi-c/../..`.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let header = bin.join("ffi_bindings").join("rorolala_ffi.h");
    let library = bin.join(format!(
        "{}rorolala{}",
        env::consts::DLL_PREFIX,
        env::consts::DLL_SUFFIX
    ));

    let mut checked = Checked::default();

    if !builds(&root, &mut checked) {
        checked.report();
        return;
    }

    checked.wants(
        "the generated header is there",
        header.is_file(),
        &format!("looked for {}", header.display()),
    );
    checked.wants(
        "the built library is there",
        library.is_file(),
        &format!("looked for {}", library.display()),
    );
    if !header.is_file() || !library.is_file() {
        checked.report();
        return;
    }

    let suite = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = env::temp_dir().join(format!("rola-ffi-c-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("a scratch directory");

    for module in modules(suite) {
        let source = suite.join(&module).join(MODULE_SOURCE);
        let executable = scratch.join(&module);

        checked.wants(
            &format!("the `{module}` C11 program compiles and answers"),
            answers(&header, suite, &bin, &source, &executable),
            "",
        );
    }

    let _ = fs::remove_dir_all(&scratch);
    checked.report();
}

/// The modules of this suite, in name order.
///
/// A module is a directory beside `src` — `ffi-*`, by convention — holding a `test.c`; anything
/// else is passed over, so adding a module is adding a directory with a program in it.
fn modules(suite: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(suite)
        .expect("the suite directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join(MODULE_SOURCE).is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();

    names.sort();
    names
}

/// Builds the library the header describes, answering whether it could.
///
/// The release programs the gate builds do not depend on the `rorolala-ffi` crate, so the artifact
/// a C caller links is not one anything else asks for; it is asked for here instead. A build that
/// already holds is cargo's to notice, so this costs nothing on a second run.
fn builds(root: &Path, checked: &mut Checked) -> bool {
    let built = Command::new(cargo())
        .args(["build", "--release", "--all-features", "-p", "rorolala-ffi"])
        .current_dir(root)
        .output();

    match built {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            checked.wants(
                "the library builds",
                false,
                &format!(
                    "cargo said:\n{}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            );
            false
        }
        Err(error) => {
            checked.wants(
                "the library builds",
                false,
                &format!("running cargo: {error}"),
            );
            false
        }
    }
}

/// Compiles one module against the header and runs it, answering whether it was happy.
///
/// The standard is C11 and every warning is an error, which is what makes the header's own
/// declarations part of what is checked: a prototype that will not compile as C fails before
/// anything runs. The module's own `test.c` says on stderr what it found wanting, and that is what
/// is shown when it is not happy.
fn answers(header: &Path, suite: &Path, bin: &Path, source: &Path, executable: &Path) -> bool {
    let bindings = header.parent().expect("the header has a directory");

    let compiled = Command::new(cc())
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg(format!("-I{}", bindings.display()))
        .arg(format!("-I{}", suite.display()))
        .arg(source)
        .arg(format!("-L{}", bin.display()))
        .arg("-lrorolala")
        .arg(format!("-Wl,-rpath,{}", bin.display()))
        .arg("-o")
        .arg(executable)
        .output();

    match compiled {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            eprint!(
                "FAIL: the C program did not compile as C11 under -Werror:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return false;
        }
        Err(error) => {
            eprintln!("FAIL: running cc: {error}");
            return false;
        }
    }

    let ran = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("running {}: {error}", executable.display()));

    if !ran.status.success() {
        eprint!("{}", String::from_utf8_lossy(&ran.stderr));
    }

    ran.status.success()
}

/// The C compiler to build with, overridable the way every toolchain here is.
fn cc() -> String {
    env::var("CC").unwrap_or_else(|_| "cc".to_owned())
}

/// The cargo to build with, overridable the way every toolchain here is.
fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// What was asked of the C ABI, and how many of the questions were answered as they should be.
#[derive(Default)]
struct Checked {
    /// How many questions were asked.
    asked: usize,
    /// Which of them were answered with something other than what was wanted.
    wrong: Vec<String>,
}

impl Checked {
    /// Counts a question, and records it as wrong when it was not answered as it should be.
    fn wants(&mut self, question: &str, answered: bool, said: &str) {
        self.asked += 1;

        if !answered {
            self.wrong.push(format!("{question} — {said}"));
        }
    }

    /// Says what was asked and what was wrong with the answers, and ends the program on them.
    fn report(self) {
        for said in &self.wrong {
            eprintln!("FAIL: {said}");
        }

        if self.wrong.is_empty() {
            println!(
                "{} questions, every one answered as it should be",
                self.asked
            );
            return;
        }

        eprintln!(
            "{} of {} answered as they should not",
            self.wrong.len(),
            self.asked
        );
        std::process::exit(1);
    }
}
