//! The `rola -V` / `rola --version` output: what this build is.
//!
//! `-V` and `--version` are not commands of their own: a `pre_dispatch` hook rewrites them to the
//! node below, which is kept out of what a run is offered — see [`VERSION_NODE`]. What is printed
//! then goes through the program's own renderers, so `--json` answers with the fields `build.rs`
//! wrote and a plain run answers with the banner.
//!
//! The values are read from the reference the workspace's build script wrote beside the target
//! directory, taken at compile time: the program carries them, rather than looking a file up again
//! at run time — a binary installed on its own has no `.cache` to read.

use mingling::{
    Grouped, StructuralData, Suggest,
    macros::{buffer, chain, command, completion, metadata, r_println, renderer, suggest},
    metadata::Description,
};
use rust_i18n::t;
use serde::{Deserialize, Serialize};

use crate::Next;

/// The node `-V` and `--version` are rewritten to before dispatch.
///
/// The name leads with `_`, which keeps it out of what a run is offered: it is the program's own
/// way of reaching this output, not a command to type. `rola _version` still reaches it, and that
/// is the node's business, not a promise.
pub const VERSION_NODE: &str = "_version";

/// The build's reference information, as the workspace's build script wrote it.
///
/// The script runs before this crate is compiled, so the file is the one this build produced rather
/// than one found again later; `include_str!` makes it a dependency of the crate, so a new commit
/// rebuilds this too.
const REFERENCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../.cache/rs-target/ref.json"
));

/// The banner the version is drawn in.
///
/// The picture is the one the project's other programs share — only the three lines beside it are
/// this program's, and they are left as markers here and filled in when it is drawn. A line is
/// written out with its leading spaces kept, so the picture lines up.
const BANNER: &str = concat!(
    "      ████████                ████████\n",
    "    ██▒▒▒▒▒▒▒▒██            ██▒▒▒▒▒▒▒▒██\n",
    "    ██        ▒▒██        ██▒▒        ██    ██████      ████    ██          ████\n",
    "    ██          ▒▒████████▒▒          ██    ██▒▒▒▒██  ██▒▒▒▒██  ██        ██▒▒▒▒██\n",
    "    ██            ▒▒▒▒▒▒▒▒            ██    ██    ██  ██    ██  ██        ██    ██\n",
    "    ██                                ██    ██████    ██    ██  ██        ████████\n",
    "    ██                                ██    ██▒▒▒▒██  ██    ██  ██        ██▒▒▒▒██\n",
    "    ██         ████      ████         ██    ██    ██  ██    ██  ██        ██    ██\n",
    "    ██         ████      ████         ██    ██    ██  ██    ██  ██        ██    ██\n",
    "    ██         ████      ████         ██    ██    ██  ██    ██  ██        ██    ██\n",
    "    ██         ▒▒▒▒      ▒▒▒▒   █     ██    ██    ██  ▒▒████▒▒  ████████  ██    ██\n",
    "    ██                         ██     ██    ▒▒    ▒▒    ▒▒▒▒    ▒▒▒▒▒▒▒▒  ▒▒    ▒▒\n",
    "    ██                ██████████      ██\n",
    "    ██                                ██    %TITLE%\n",
    "      ████████████████████████████████      %VERSION_LINE%\n",
    "      ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒      %BUILD_LINE%",
);

/// How much of a commit's hash is shown beside the version.
///
/// A hash is long enough to name a commit and longer than a line wants, so what is drawn is the
/// head of it — the same seven characters git shows.
const COMMIT_HEAD: usize = 7;

/// The fields the reference holds, as the build script wrote them.
#[derive(Deserialize)]
struct Reference {
    commit_date: String,
    commit_hash: String,
    rustc_version: String,
    version: String,
}

#[metadata(EntryVersion)]
pub fn desc_version() -> Description {
    t!("version.cmd_version_description").to_string().into()
}

/// Completes what the version output can be given next.
///
/// The node names nothing, and it is reached by the program rather than typed, so there is nothing
/// to offer.
#[completion(EntryVersion)]
pub fn complete_version() -> Suggest {
    suggest!()
}

/// Prints what this build is
///
/// `-V` and `--version` reach this, and so does `rola _version`; nothing is named, since the build
/// is what it is.
#[command(node = "_version")]
pub fn version() -> StateVersion {
    StateVersion
}

/// The state the version output starts in: nothing to name.
#[derive(Grouped)]
pub struct StateVersion;

#[chain]
pub fn handle_version(_state: StateVersion) -> Next {
    ResultVersion::of_the_build().into()
}

/// Result: what this build is.
///
/// The fields are the reference's own, so a `--json` run is answered with what the build wrote
/// rather than with the line a reader would have been shown.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVersion {
    /// When the commit the build is made from was made.
    commit_date: String,
    /// The commit the build is made from.
    commit_hash: String,
    /// The rustc the build was made with.
    rustc_version: String,
    /// The version the build is.
    version: String,
}

impl ResultVersion {
    /// The reference, read back.
    ///
    /// A reference that does not read is not a failure to stop a run over: what it is for is saying
    /// what the build is, so the version falls back to the crate's own and the rest is left empty
    /// rather than a program that cannot say its own version.
    fn of_the_build() -> Self {
        serde_json::from_str::<Reference>(REFERENCE).map_or_else(
            |_| Self {
                commit_date: String::new(),
                commit_hash: String::new(),
                rustc_version: String::new(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            |reference| Self {
                commit_date: reference.commit_date,
                commit_hash: reference.commit_hash,
                rustc_version: reference.rustc_version,
                version: reference.version,
            },
        )
    }

    /// The commit's head, as it is drawn beside the version.
    fn commit_head(&self) -> &str {
        self.commit_hash
            .get(..COMMIT_HEAD)
            .unwrap_or(&self.commit_hash)
    }
}

#[renderer(buffer)]
pub fn render_result_version(result: ResultVersion) {
    let version_line = format!("rola {} ({})", result.version, result.commit_head());
    let build_line = format!("{} · {}", result.commit_date, result.rustc_version);

    let banner = BANNER
        .replace("%TITLE%", t!("version.title").trim())
        .replace("%VERSION_LINE%", &version_line)
        .replace("%BUILD_LINE%", &build_line);

    r_println!("{banner}");
}
