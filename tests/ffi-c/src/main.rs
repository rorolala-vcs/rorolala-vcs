//! `ffi-c`: the C ABI, compiled and run as C11 programs.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: what
//! a C caller does is compile against the generated header, link the built library, run, and read
//! what came back. Nothing Rust reaches across the boundary here, the checks are the ones a C
//! program can make, made in C, so what is checked is the ABI itself rather than the library under
//! it.
//!
//! The checks are split into modules, one directory beside this one per area of the surface, each
//! named `ffi-<area>` and holding a `test.c` that is a C11 program of its own. Every directory with
//! such a program is compiled and run, so what is answered here is one question per module, and
//! one more for the library and the header being there at all.
//!
//! The library is built here rather than waited for, since the release programs do not depend on the
//! `rorolala-ffi` crate, a header without the library beside it proves nothing.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use librorolala::layout::{LayoutPath, MutableData};
use librorolala::vault::Vault;
use librorolala::workspace::{Config as WorkspaceConfig, Workspace};
use rorolala_book_search::{SourceArticle, encode};
use rorolala_utils_configure::Configure as _;
use rorolala_utils_location::Locate as _;
use rorolala_utils_sandbox::bin_dir;
use uuid::Uuid;

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

    // The book the `ffi-book` program searches. It is made here rather than committed, so what a
    // caller searches is what the crate writes, and a format change cannot leave a fixture behind
    // that no longer means what it said.
    let fixture = fixture(&scratch);

    for module in modules(suite) {
        let source = suite.join(&module).join(MODULE_SOURCE);
        let executable = scratch.join(&module);

        checked.wants(
            &format!("the `{module}` C11 program compiles and answers"),
            answers(&header, suite, &bin, &source, &executable, &fixture),
            "",
        );
    }

    cross_checks(&fixture, &bin, &mut checked);

    let _ = fs::remove_dir_all(&scratch);
    checked.report();
}

/// What the library writes and what the command writes are one document.
///
/// The ABI's answers are the JSON the commands write, which is what lets a caller parse one
/// document whichever side answered it. The checks here are the promise made where the exports are
/// declared: the fixture Workspace is read both ways, and the two documents are compared as values
/// rather than as text, so key order is not a difference.
fn cross_checks(fixture: &Fixture, bin: &Path, checked: &mut Checked) {
    let workspace = &fixture.workspace;

    checked.wants(
        "the Layouts the library writes are the ones the command writes",
        same(
            bin,
            workspace,
            &["--json", "layout", "ls"],
            &librorolala::layouts_json(workspace),
            None,
        ),
        "",
    );

    let entries = librorolala::layout_entries_json(workspace, "").unwrap_or_default();

    checked.wants(
        "the entries the library writes are the ones the command writes",
        same(
            bin,
            workspace,
            &["--json", "layout", "entries"],
            &entries,
            None,
        ),
        "",
    );

    checked.wants(
        "the Vault bindings the library writes are the ones the command writes",
        same(
            bin,
            workspace,
            &["--json", "vault", "ls"],
            &librorolala::workspace_config_json(workspace),
            Some("vaults"),
        ),
        "",
    );
}

/// Whether the library's answer and the command's are one document.
///
/// `key` names the member both documents are compared by, for a read whose answer carries more than
/// the command's does; `None` compares the whole document.
fn same(
    bin: &Path,
    directory: &Path,
    arguments: &[&str],
    library: &str,
    key: Option<&str>,
) -> bool {
    let program = bin.join(format!("rola{}", env::consts::EXE_SUFFIX));
    let Ok(output) = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .output()
    else {
        return false;
    };

    if !output.status.success() {
        return false;
    }

    let (Ok(library), Ok(command)) = (
        serde_json::from_str::<serde_json::Value>(library),
        serde_json::from_slice::<serde_json::Value>(&output.stdout),
    ) else {
        return false;
    };

    match key {
        Some(key) => library[key] == command[key],
        None => library == command,
    }
}

/// The places a C program reads: the book it searches, the Workspace and Vault it lists, and the tree it marks.
struct Fixture {
    /// The book file itself, which is opened directly.
    book: PathBuf,
    /// A directory beside the book's, as a program's own directory would be.
    beside: PathBuf,
    /// A Workspace working in a Layout that names entries.
    workspace: PathBuf,
    /// A Vault whose own Layout names an entry.
    vault: PathBuf,
    /// The root of a tree, with an id and a staged description.
    tree: PathBuf,
    /// The key the fixture Workspace's store holds some content under.
    storage_key: String,
    /// A directory the C programs may write into, which is the scratch this run made.
    scratch: PathBuf,
}

/// Writes the fixture book and the directory it is located from.
fn fixture(scratch: &Path) -> Fixture {
    let docs = scratch.join("docs");
    fs::create_dir_all(&docs).expect("the book's directory");

    let articles = vec![
        SourceArticle {
            name: "needle".to_owned(),
            title: "The Needle Article".to_owned(),
            description: "A haystack with a needle in it".to_owned(),
            keywords: vec!["needle".to_owned()],
            markdown: "A haystack with a needle buried somewhere in the straw".to_owned(),
            plain: "A haystack with a needle buried somewhere in the straw".to_owned(),
        },
        SourceArticle {
            name: "haystack".to_owned(),
            title: "A Haystack".to_owned(),
            description: "Some straw".to_owned(),
            keywords: Vec::new(),
            markdown: "A haystack with no metal in it at all".to_owned(),
            plain: "A haystack with no metal in it at all".to_owned(),
        },
    ];

    let book = docs.join("builtin.book");
    let bytes = encode(&articles).expect("the fixture book encodes");
    fs::write(&book, bytes).expect("the fixture book is written");

    let beside = scratch.join("bin");
    fs::create_dir_all(&beside).expect("a directory beside the book's");

    let workspace = workspace_fixture(scratch);
    let storage_key = storage_fixture(&workspace, scratch);

    Fixture {
        book,
        beside,
        workspace,
        vault: vault_fixture(scratch),
        tree: tree_fixture(scratch),
        storage_key,
        scratch: scratch.to_path_buf(),
    }
}

/// Puts one small file in the fixture Workspace's store, answering the key it is kept under.
fn storage_fixture(workspace: &Path, scratch: &Path) -> String {
    let located = Workspace::locate(workspace).expect("the fixture Workspace locates");
    let store = located
        .get_current_rola_storage()
        .expect("the fixture Workspace has a store");
    let file = scratch.join("fixture-content.bin");
    fs::write(&file, b"fixture content").expect("the fixture content is written");

    let runtime = tokio::runtime::Runtime::new().expect("a runtime of the fixture's own");

    runtime
        .block_on(librorolala::storage::store_file(&store, &file))
        .expect("the fixture content is stored")
        .to_string()
}

/// A Workspace working in a Layout that names two entries, one of them deprecated.
///
/// The Vaults it knows are bound as well, so the configuration read has something to list: a
/// binding is the one thing a Workspace's configuration holds that a caller can see.
fn workspace_fixture(scratch: &Path) -> PathBuf {
    let dir = scratch.join("workspace");
    Workspace::create(&dir).expect("the fixture Workspace is made");

    let workspace = Workspace::locate(&dir).expect("the fixture Workspace locates");
    let layout = workspace.layout().expect("the fixture Layout opens");

    layout
        .create_entry(
            Uuid::from_u128(1),
            MutableData::new(Some("alice".to_owned()), [1; 32], "the hero".to_owned()),
        )
        .expect("the fixture entry is made");
    layout
        .create_path(
            &LayoutPath::new("art/hero.psd").expect("a path"),
            Uuid::from_u128(1),
        )
        .expect("the fixture path is named");
    layout
        .create_entry(
            Uuid::from_u128(2),
            MutableData::new(None, [2; 32], String::new()),
        )
        .expect("the deprecated fixture entry is made");
    layout
        .create_path(
            &LayoutPath::new("@/removed/old.psd").expect("a path"),
            Uuid::from_u128(2),
        )
        .expect("the deprecated fixture path is named");

    let path = workspace.config_path();
    let mut config = WorkspaceConfig::read_from(&path).expect("the fixture configuration reads");
    config.vaults_mut().bind("studio", "tcp://127.0.0.1:9270");
    config
        .vaults_mut()
        .bind("alpha", "ssh://alice@example.test/vault");
    config.default_config_mut().set_vault("studio");
    config
        .write_to(&path)
        .expect("the fixture configuration writes");

    dir
}

/// A Vault whose own Layout names one entry.
fn vault_fixture(scratch: &Path) -> PathBuf {
    let dir = scratch.join("vault");
    Vault::create(&dir).expect("the fixture Vault is made");

    let vault = Vault::locate(&dir).expect("the fixture Vault locates");
    let layout = vault.layout().expect("the fixture Vault Layout opens");

    layout
        .create_entry(
            Uuid::from_u128(3),
            MutableData::new(
                Some("bob".to_owned()),
                [3; 32],
                "the vault's file".to_owned(),
            ),
        )
        .expect("the Vault's fixture entry is made");
    layout
        .create_path(
            &LayoutPath::new("shared.psd").expect("a path"),
            Uuid::from_u128(3),
        )
        .expect("the Vault's fixture path is named");

    dir
}

/// The root of a tree, with an id and a description staged for the next version.
fn tree_fixture(scratch: &Path) -> PathBuf {
    let dir = scratch.join("tree");
    let mark = dir.join(".rola-tree");
    fs::create_dir_all(&mark).expect("the tree's mark");
    fs::write(mark.join("id"), "tree-id").expect("the tree's id");
    fs::write(mark.join("description"), "what the next version says").expect("the tree's draft");

    dir
}

/// The modules of this suite, in name order.
///
/// A module is a directory beside `src` (`ffi-*`, by convention) holding a `test.c`; anything
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

/// The environment variable a module is handed the fixture book's path in.
const BOOK_VARIABLE: &str = "ROLA_BOOK";

/// The environment variable a module is handed the directory to locate the book from in.
const LOCATE_VARIABLE: &str = "ROLA_BOOK_LOCATE";

/// The environment variable a module is handed the fixture Workspace's path in.
const WORKSPACE_VARIABLE: &str = "ROLA_WORKSPACE";

/// The environment variable a module is handed the fixture Vault's path in.
const VAULT_VARIABLE: &str = "ROLA_VAULT";

/// The environment variable a module is handed the fixture tree's path in.
const TREE_VARIABLE: &str = "ROLA_TREE";

/// The environment variable a module is handed the key the fixture store holds content under.
const STORAGE_KEY_VARIABLE: &str = "ROLA_STORAGE_KEY";

/// The environment variable a module is handed a directory it may write into.
const SCRATCH_VARIABLE: &str = "ROLA_SCRATCH";

/// Compiles one module against the header and runs it, answering whether it was happy.
///
/// The standard is C11 and every warning is an error, which is what makes the header's own
/// declarations part of what is checked: a prototype that will not compile as C fails before
/// anything runs. The module's own `test.c` says on stderr what it found wanting, and that is what
/// is shown when it is not happy. The fixture book is named to the program in the environment
/// rather than compiled into it, so the same source works from a checkout in any directory.
fn answers(
    header: &Path,
    suite: &Path,
    bin: &Path,
    source: &Path,
    executable: &Path,
    fixture: &Fixture,
) -> bool {
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
        .env(BOOK_VARIABLE, &fixture.book)
        .env(LOCATE_VARIABLE, &fixture.beside)
        .env(WORKSPACE_VARIABLE, &fixture.workspace)
        .env(VAULT_VARIABLE, &fixture.vault)
        .env(TREE_VARIABLE, &fixture.tree)
        .env(STORAGE_KEY_VARIABLE, &fixture.storage_key)
        .env(SCRATCH_VARIABLE, &fixture.scratch)
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
            self.wrong.push(format!("{question}, {said}"));
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
