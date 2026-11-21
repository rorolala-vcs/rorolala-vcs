//! `fonts`: the face the Desktop program is drawn in, fetched at the pinned Noto Sans commit.

use std::path::Path;

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::fs;
use crate::paths::Workspace;
use crate::run_step;
use crate::toolchain::Tool;

/// Where the Noto Sans set is kept, from the root of the tree.
pub const FOLDER: &str = "THIRD_PARTY/Fonts/NotoSans";

/// The commit every font file is fetched from.
///
/// The commit rather than the branch, because a branch is a moving answer and what the program is
/// drawn in has to be one face: a build that fetched a different commit every day would be a program
/// that looked different every day. The stamp beside the files is what tells a fetched set from one
/// that has to be fetched again.
pub const COMMIT: &str = "ffebf8c1ee449e544955a7e813c54f9b73848eac";

/// Where the set comes from.
const RAW: &str = "https://raw.githubusercontent.com/notofonts/noto-fonts";

/// The files the programs are drawn with: the weights the look sets words in, and the italics a log
/// or an editor may ask for.
const FONTS: [&str; 6] = [
    "hinted/ttf/NotoSans/NotoSans-Regular.ttf",
    "hinted/ttf/NotoSans/NotoSans-Italic.ttf",
    "hinted/ttf/NotoSans/NotoSans-SemiBold.ttf",
    "hinted/ttf/NotoSans/NotoSans-Bold.ttf",
    "hinted/ttf/NotoSans/NotoSans-ExtraBold.ttf",
    "hinted/ttf/NotoSans/NotoSans-BoldItalic.ttf",
];

/// The license the set is distributed under.
const LICENSE: &str = "LICENSE";

/// The file the commit the set was fetched at is recorded in.
const STAMP: &str = "COMMIT";

/// Fetches the Noto Sans files the programs are drawn in, where the build reads them.
///
/// A set that records the pinned commit and holds every file is left as it is, so a build that already
/// has it says nothing and reaches the network for nothing. What is fetched is laid under
/// [`FOLDER`], and the same directory is laid beside the programs by [`lay_out_fonts`].
///
/// # Errors
///
/// Reports a directory or a download that failed.
pub fn fetch_fonts(workspace: &Workspace) -> Result<(), crate::Error> {
    let folder = workspace.root().join(FOLDER);

    if settled(&folder) {
        return Ok(());
    }

    fs::remove(&folder)?;
    fs::create_directory(&folder)?;

    for file in FONTS.into_iter().chain([LICENSE]) {
        download(&folder, file)?;
    }

    std::fs::write(folder.join(STAMP), COMMIT).map_err(|error| crate::Error::Io {
        doing: "recording the Noto Sans commit",
        detail: format!("{}: {error}", folder.display()),
    })?;

    Ok(())
}

/// Lays the fetched set beside the programs, with the license it is distributed under.
///
/// # Errors
///
/// Reports a directory or a copy that failed.
pub fn lay_out_fonts(workspace: &Workspace, desktop: &Path) -> Result<(), crate::Error> {
    fs::copy_directory(&workspace.root().join(FOLDER), &desktop.join(FOLDER))
}

/// Whether a directory holds the pinned commit and every file it names.
///
/// A file that is there but empty is not there: a download that was cut short leaves one.
fn settled(folder: &Path) -> bool {
    let stamp = std::fs::read_to_string(folder.join(STAMP)).unwrap_or_default();

    stamp.trim() == COMMIT
        && FONTS.into_iter().chain([LICENSE]).all(|file| {
            std::fs::metadata(folder.join(name(file)))
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
        })
}

/// The file a path in the set names.
fn name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Fetches one file of the set, named by its path in the repository.
fn download(folder: &Path, path: &str) -> Result<(), crate::Error> {
    let url = format!("{RAW}/{COMMIT}/{path}");
    let destination = folder.join(name(path));
    let destination = destination.to_string_lossy();

    Tool::curl().run([
        "-fsSL",
        "--retry",
        "3",
        "--retry-delay",
        "2",
        "--output",
        destination.as_ref(),
        url.as_str(),
    ])
}

/// Fetches the Noto Sans files the programs are drawn in.
#[command(node = "fonts")]
pub fn fonts() -> StateFonts {
    StateFonts
}

/// The state the command starts in: the one fetched set is the whole of what it names.
#[derive(Grouped)]
pub struct StateFonts;

/// Fetches the set, where the build reads it.
#[chain]
pub fn handle_fonts(_state: StateFonts, ec: &mut ResExitCode) -> Next {
    run_step(ec, || fetch_fonts(&Workspace::discover()?));

    empty_result!()
}
