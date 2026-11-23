//! `ffi-cxx`: the C++ layer, compiled and run as C++23 programs.
//!
//! A program rather than a set of tests cargo runs, for the same reason the C ABI's suite is one:
//! what a C++ caller does is include the umbrella header, link the built library, run, and read what
//! came back. The checks are the ones a C++ program can make, made in C++, so what is checked is the
//! layer a consumer compiles rather than the Rust underneath it.
//!
//! The generated headers are not in the tree, so this suite lays the include tree out itself: the
//! hand-written headers, the generator's output beside them, and the C header under the root the
//! generated files include it from. Each module is a directory beside this one holding a
//! `test.cpp`, and every such directory is compiled and run, so adding a module is adding a
//! directory with a program in it.
//!
//! The library is built here rather than waited for, since the release programs do not depend on
//! the `rororala-ffi` crate, and a header without the library beside it proves nothing.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ed25519_dalek::SigningKey;
use librorolala::layout::{LayoutPath, MutableData};
use librorolala::workspace::{Config as WorkspaceConfig, Workspace};
use pkcs8::{EncodePrivateKey as _, LineEnding};
use rorolala_utils_configure::Configure as _;
use rorolala_utils_location::Locate as _;
use rorolala_utils_sandbox::bin_dir;
use uuid::Uuid;

/// The file a module holds its own C++ program in.
const MODULE_SOURCE: &str = "test.cpp";

fn main() {
    let bin = bin_dir();
    // The workspace root as seen from this suite: `tests/ffi-cxx/../..`.
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
    let scratch = env::temp_dir().join(format!("rola-ffi-cxx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("a scratch directory");

    let include = scratch.join("include");
    if !prepares(&root, &header, &include, &mut checked) {
        let _ = fs::remove_dir_all(&scratch);
        checked.report();
        return;
    }

    let fixture = fixture(&scratch);

    for module in modules(suite) {
        let source = suite.join(&module).join(MODULE_SOURCE);
        let executable = scratch.join(&module);

        checked.wants(
            &format!("the `{module}` C++ program compiles and answers"),
            answers(&include, &bin, &source, &executable, &fixture),
            "",
        );
    }

    let _ = fs::remove_dir_all(&scratch);
    checked.report();
}

/// The places a C++ program reads: the Workspace it is opened from, the tree it marks, the content
/// the store holds, and the account a daemon request is made as.
struct Fixture {
    /// A Workspace working in a Layout that names entries, with a store and an account.
    workspace: PathBuf,
    /// The root of a tree, with an id and a staged description.
    tree: PathBuf,
    /// The key the fixture Workspace's store holds some content under.
    storage_key: String,
    /// The name of the account the fixture key gives the run.
    account: String,
    /// A directory the C++ programs may write into, which is the scratch this run made.
    scratch: PathBuf,
}

/// Writes the fixture Workspace, tree and account.
fn fixture(scratch: &Path) -> Fixture {
    let workspace = workspace_fixture(scratch);
    let storage_key = storage_fixture(&workspace, scratch);

    Fixture {
        account: account_fixture(&workspace),
        workspace,
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

/// The root of a tree, with an id and a description staged for the next version.
fn tree_fixture(scratch: &Path) -> PathBuf {
    let dir = scratch.join("tree");
    let mark = dir.join(".rola-tree");
    fs::create_dir_all(&mark).expect("the tree's mark");
    fs::write(mark.join("id"), "tree-id").expect("the tree's id");
    fs::write(mark.join("description"), "what the next version says").expect("the tree's draft");

    dir
}

/// An account inside the fixture Workspace, answering the name it is found by.
///
/// An account is a private key file, named by its stem, under the Workspace's keys directory. The
/// key is made from a fixed seed so the fixture is the same on every run; a deterministic key is
/// what the auth crate's own tests use as well.
fn account_fixture(workspace: &Path) -> String {
    let keys = workspace.join(".rola").join("auth");
    fs::create_dir_all(&keys).expect("the Workspace's key directory");

    let key = SigningKey::from_bytes(&[7u8; 32]);
    let pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .expect("a private key encodes as PKCS#8");
    fs::write(keys.join("alice.pem"), pem.as_bytes()).expect("the fixture account key");

    "alice".to_owned()
}

/// The modules of this suite, in name order.
///
/// A module is a directory beside `src` holding a `test.cpp`; anything else is passed over, so
/// adding a module is adding a directory with a program in it.
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
/// The release programs the gate builds do not depend on the `rororala-ffi` crate, so the artifact
/// a C++ caller links is not one anything else asks for; it is asked for here instead. A build that
/// already holds is cargo's to notice, so this costs nothing on a second run.
fn builds(root: &Path, checked: &mut Checked) -> bool {
    let built = Command::new(cargo())
        .args(["build", "--release", "--all-features", "-p", "rororala-ffi"])
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

/// Lays the include tree out from the module's headers and the generator, answering whether it did.
///
/// The generator is the tree's own program, run the way `cargo dev export` runs it, so a generated
/// header that does not compile fails here rather than in a consumer's build.
fn prepares(root: &Path, header: &Path, include: &Path, checked: &mut Checked) -> bool {
    let written = root.join("modules").join("rola-plus-plus").join("include");

    if let Err(error) = copy_directory(&written, include) {
        checked.wants("the hand-written headers are laid out", false, &error);
        return false;
    }

    if let Err(error) = fs::copy(header, include.join("rorolala.h")) {
        checked.wants(
            "the C header is laid out under the include root",
            false,
            &format!("{}: {error}", header.display()),
        );
        return false;
    }

    let generated = Command::new(cargo())
        .current_dir(root)
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .arg("-p")
        .arg("rororala-dev-cpp-bindgen")
        .arg("--bin")
        .arg("cpp-bindgen")
        .arg("--")
        .arg(header)
        .arg(include.join("rola_plus_plus").join("gen"))
        .arg("--umbrella")
        .arg(
            include
                .join("rola_plus_plus")
                .join("gen")
                .join("rola_plus_plus.gen.hpp"),
        )
        .output();

    match generated {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            checked.wants(
                "the generated headers are laid out",
                false,
                &format!(
                    "cpp-bindgen said:\n{}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            );
            false
        }
        Err(error) => {
            checked.wants(
                "the generated headers are laid out",
                false,
                &format!("running cpp-bindgen: {error}"),
            );
            false
        }
    }
}

/// Copies a directory and everything under it.
fn copy_directory(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| format!("{}: {error}", to.display()))?;

    for entry in fs::read_dir(from).map_err(|error| format!("{}: {error}", from.display()))? {
        let entry = entry.map_err(|error| format!("{}: {error}", from.display()))?;
        let path = entry.path();
        let destination = to.join(entry.file_name());

        if path.is_dir() {
            copy_directory(&path, &destination)?;
        } else {
            fs::copy(&path, &destination)
                .map_err(|error| format!("{}: {error}", destination.display()))?;
        }
    }

    Ok(())
}

/// The environment variable a module is handed the fixture Workspace's path in.
const WORKSPACE_VARIABLE: &str = "ROLA_WORKSPACE";

/// The environment variable a module is handed the fixture tree's path in.
const TREE_VARIABLE: &str = "ROLA_TREE";

/// The environment variable a module is handed the key the fixture store holds content under.
const STORAGE_KEY_VARIABLE: &str = "ROLA_STORAGE_KEY";

/// The environment variable a module is handed the fixture account's name in.
const ACCOUNT_VARIABLE: &str = "ROLA_ACCOUNT";

/// The environment variable a module is handed a directory it may write into.
const SCRATCH_VARIABLE: &str = "ROLA_SCRATCH";

/// Compiles one module against the laid-out headers and runs it, answering whether it was happy.
///
/// The standard is C++23 and every warning is an error, which is what makes the headers part of what
/// is checked: a declaration that will not compile as C++ fails before anything runs. The module's
/// own `test.cpp` says on stderr what it found wanting, and that is what is shown when it is not
/// happy. The fixtures are named to the program in the environment rather than compiled into it, so
/// the same source works from a checkout in any directory.
fn answers(
    include: &Path,
    bin: &Path,
    source: &Path,
    executable: &Path,
    fixture: &Fixture,
) -> bool {
    let compiled = Command::new(cxx())
        .arg("-std=c++23")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg(format!("-I{}", include.display()))
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
                "FAIL: the C++ program did not compile as C++23 under -Werror:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return false;
        }
        Err(error) => {
            eprintln!("FAIL: running c++: {error}");
            return false;
        }
    }

    let ran = Command::new(executable)
        .env(WORKSPACE_VARIABLE, &fixture.workspace)
        .env(TREE_VARIABLE, &fixture.tree)
        .env(STORAGE_KEY_VARIABLE, &fixture.storage_key)
        .env(ACCOUNT_VARIABLE, &fixture.account)
        .env(SCRATCH_VARIABLE, &fixture.scratch)
        .output()
        .unwrap_or_else(|error| panic!("running {}: {error}", executable.display()));

    if !ran.status.success() {
        eprint!("{}", String::from_utf8_lossy(&ran.stderr));
    }

    ran.status.success()
}

/// The C++ compiler to build with, overridable the way every toolchain here is.
fn cxx() -> String {
    env::var("CXX").unwrap_or_else(|_| "c++".to_owned())
}

/// The cargo to build with, overridable the way every toolchain here is.
fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// What was asked of the C++ layer, and how many of the questions were answered as they should be.
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
