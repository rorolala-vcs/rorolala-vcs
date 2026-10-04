//! Draws the Desktop's own pictures from the pinned Material Design icon set.
//!
//! What is drawn is what `app/desktop/icons.toml` names: the commit to draw from, and the pictures each
//! plugin wears. What is written beside them is what a directory of pictures has to say for itself — the
//! license the artwork is under, where it came from, and how to draw it again — and the C# keys the plugin
//! reads them by. Adding a picture is therefore a line in the manifest rather than an edit in four places.
//!
//! This is a development tool: it is crude on purpose, and it is what runs, not the interface.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::Deserialize;

/// Where the manifest is, from the root of the tree.
const MANIFEST: &str = "app/desktop/icons.toml";

/// Where the icon set is kept, from the root of the tree.
const CLONE: &str = "THIRD-PARTY/material-icons";

/// Where the plugins are, from the root of the tree.
const PLUGINS: &str = "app/desktop/Plugins";

/// What a picture is drawn in, and how many pixels wide and tall it is.
///
/// The ink is white and the ground transparent, because a plugin draws a picture as a mask over a colour it
/// takes from the theme: white is opaque under either reading of a mask — its alpha, and its luminance — so
/// the mark takes the colour the user is looking at rather than one baked into the file. The size is four
/// times the set's own 24-unit grid, which is as large as a tile's icon ever gets.
const INK: &str = "#FFFFFF";
const SIZE: u32 = 96;

/// How wide a picture is, as the number a transform is scaled by.
///
/// `SIZE` is small enough to be a float exactly — a picture is drawn at whole pixels and scaled by a fraction
/// of the grid it was drawn on — so the lossy-looking cast is the whole of what is wanted.
const fn wide() -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let wide = SIZE as f32;

    wide
}

/// What every picture of one plugin is drawn for.
#[derive(Deserialize)]
struct Set {
    /// The plugin, as its directory under `app/desktop/Plugins` and as the namespace of its keys.
    plugin: String,
    /// What the pictures are handed over under.
    key: String,
    /// The pictures, by the name the set knows each by.
    icons: Vec<String>,
}

/// The set to draw from.
#[derive(Deserialize)]
struct Material {
    /// Where the set is cloned from.
    url: String,
    /// The commit every picture is drawn from.
    commit: String,
}

/// The manifest: the set, and the pictures each plugin wears.
#[derive(Deserialize)]
struct Manifest {
    material: Material,
    set: Vec<Set>,
}

fn main() -> ExitCode {
    let root = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);

    match draw(&root) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Draws every picture every set names, and writes what goes beside them.
fn draw(root: &Path) -> Result<(), String> {
    let manifest = fs::read_to_string(root.join(MANIFEST))
        .map_err(|error| format!("the manifest could not be read: {error}"))?;

    let manifest: Manifest = toml::from_str(&manifest)
        .map_err(|error| format!("the manifest could not be read: {error}"))?;

    println!("==> icons at {}", manifest.material.commit);
    ensure_set(root, &manifest.material)?;

    let license = fs::read_to_string(root.join(CLONE).join("LICENSE"))
        .map_err(|error| format!("the set's license could not be read: {error}"))?;

    for set in &manifest.set {
        let pictures = root.join(PLUGINS).join(&set.plugin).join("icons");
        fs::create_dir_all(&pictures)
            .map_err(|error| format!("`{}` could not be made: {error}", pictures.display()))?;

        for icon in &set.icons {
            draw_picture(root, &pictures, icon)?;
        }

        fs::write(pictures.join("LICENSE"), &license)
            .map_err(|error| format!("the license could not be written: {error}"))?;

        let notice = pictures.join("README.md");
        fs::write(&notice, said(&manifest.material, set))
            .map_err(|error| format!("`{}` could not be written: {error}", notice.display()))?;
        println!("==> {}", notice.display());

        let keys = root
            .join(PLUGINS)
            .join(&set.plugin)
            .join("Icons.Generated.cs");
        fs::write(&keys, keys_of(set))
            .map_err(|error| format!("`{}` could not be written: {error}", keys.display()))?;
        println!("==> {}", keys.display());
    }

    Ok(())
}

/// Draws one picture, by the name the set knows it by.
fn draw_picture(root: &Path, pictures: &Path, icon: &str) -> Result<(), String> {
    let source = root.join(CLONE).join("svg").join(icon).join("baseline.svg");
    let text = fs::read_to_string(&source).map_err(|error| {
        format!(
            "the icon set holds no `{icon}`: `{}` could not be read: {error}",
            source.display()
        )
    })?;

    // The set draws in black, so the ink is set on the way through rather than left to the renderer, which
    // has no colour of its own to be told.
    let inked = text.replace("<path ", &format!("<path fill=\"{INK}\" "));

    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(&inked, &options)
        .map_err(|error| format!("`{icon}` could not be read as a picture: {error}"))?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE, SIZE)
        .ok_or_else(|| format!("a {SIZE}-pixel square could not be made for `{icon}`"))?;

    // The set's own grid scaled to the size asked for, so that what is drawn is the whole of the artwork.
    let scale = wide() / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let picture = pictures.join(format!("{icon}.png"));
    pixmap
        .save_png(&picture)
        .map_err(|error| format!("`{}` could not be written: {error}", picture.display()))?;
    println!("==> {}", picture.display());

    Ok(())
}

/// Puts the clone where this expects it: the named commit, and nothing changed in it.
fn ensure_set(root: &Path, material: &Material) -> Result<(), String> {
    let clone = root.join(CLONE);

    if !clone.join(".git").is_dir() {
        println!("==> cloning {}", material.url);
        let _ = fs::remove_dir_all(&clone);
        fs::create_dir_all(&clone)
            .map_err(|error| format!("`{}` could not be made: {error}", clone.display()))?;
        git(&clone, &["init", "-q"])?;
        git(&clone, &["remote", "add", "origin", &material.url])?;
    }

    if git_says(&clone, &["rev-parse", "HEAD"]).trim() != material.commit {
        println!("==> checking out {}", material.commit);
        git(
            &clone,
            &["fetch", "-q", "--depth", "1", "origin", &material.commit],
        )?;
        git(&clone, &["checkout", "-q", "--detach", &material.commit])?;
    }

    if !git_says(&clone, &["status", "--porcelain"])
        .trim()
        .is_empty()
    {
        println!("==> restoring {}", clone.display());
        git(&clone, &["checkout", "-q", "--force", &material.commit])?;
        git(&clone, &["clean", "-qffd"])?;
    }

    Ok(())
}

/// Runs git in the clone, failing when it does.
fn git(repository: &Path, arguments: &[&str]) -> Result<(), String> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .status()
        .map_err(|error| format!("git could not be run: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("git {} failed", arguments.join(" ")))
    }
}

/// Runs git in the clone and hands back what it said, saying nothing when it failed.
fn git_says(repository: &Path, arguments: &[&str]) -> String {
    Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(String::new, |output| {
            String::from_utf8_lossy(&output.stdout).into_owned()
        })
}

/// What a directory of pictures says for itself: where they came from, and what to do to draw them again.
fn said(material: &Material, set: &Set) -> String {
    let list = set
        .icons
        .iter()
        .map(|icon| format!("`{icon}`"))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "# The {} pictures\n\
         \n\
         The pictures the `{}` plugin draws: {}.\n\
         \n\
         They are Google's Material Design icons, taken from the maintained mirror of the set,\n\
         [material-icons/material-icons]({}) — the same artwork as\n\
         [google/material-design-icons](https://github.com/google/material-design-icons) — under the\n\
         [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0). The license itself is\n\
         `LICENSE`, copied from the commit below.\n\
         \n\
         **The set is not vendored here.** It is a shallow clone under `THIRD-PARTY/material-icons/`, which\n\
         git ignores, and the pictures are drawn from it by `./run.sh icons` rather than copied by hand, so\n\
         that what is drawn is what the pinned commit holds. The manifest is `app/desktop/icons.toml`:\n\
         another picture is another name there rather than another file in this directory.\n\
         \n\
         - drawn from: `{}`\n\
         - handed over under: `{}.*`\n\
         \n\
         **Nothing here is edited by hand.** The pictures, this file and the license are what that run\n\
         writes; a change made here is a change made nowhere.\n\
         \n\
         **Why white.** A picture is not drawn as a picture: a plugin draws it as an opacity mask over a\n\
         ground it colours from the theme, so the mark takes the ink the user is looking at rather than a\n\
         colour baked into the file. White is opaque under either reading of a mask — its alpha, and its\n\
         luminance — so the picture works whichever one the toolkit does.\n\
         \n\
         **Why raster rather than linked.** Nothing at runtime draws an SVG: the toolkit would need another\n\
         package for it, and a plugin is discovered as a single assembly. The pictures are drawn at {SIZE}\n\
         pixels, which is four times the 24-unit viewBox the artwork is drawn at — as large as a tile's icon\n\
         ever gets at the top of the zoom, so the mask is scaled down rather than up.\n",
        set.plugin, set.plugin, list, material.url, material.commit, set.key
    )
}

/// The C# keys a plugin reads its pictures by, generated so that no one writes them twice.
fn keys_of(set: &Set) -> String {
    let mut keys = String::new();
    let mut table = String::new();

    for icon in &set.icons {
        let named = constant(icon);

        // Written into the string rather than formatted into a second one and appended: this is a file being
        // written, and one `String` is what it is written into.
        let _ = write!(
            keys,
            "    /// <summary>The key the `{icon}` picture is held under.</summary>\n    \
             public const string {named} = \"{}.{icon}\";\n\n",
            set.key
        );
        let _ = writeln!(table, "        ({named}, \"{icon}\"),");
    }

    format!(
        "// Generated by `rorolala-dev-icons` from `app/desktop/icons.toml`.\n\
         //\n\
         // Do not edit: what this holds is what the manifest says, and a change made here is a change made\n\
         // nowhere — the next `./run.sh icons` would take it away again.\n\
         \n\
         namespace {};\n\
         \n\
         /// <summary>The pictures this plugin contributes, and the key each is handed over under.</summary>\n\
         internal static partial class Icons\n\
         {{\n\
         {keys}\
         \x20   /// <summary>Every picture, by the key it is held under and its name in the set.</summary>\n\
         \x20   public static readonly (string Key, string Name)[] Pictures =\n\
         \x20   [\n\
         {table}\
         \x20   ];\n\
         }}\n",
        set.plugin
    )
}

/// The name a picture's key constant is written as, from the name the set knows it by.
fn constant(icon: &str) -> String {
    icon.split('_')
        .map(|part| {
            let mut letters = part.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + letters.as_str()
            })
        })
        .collect()
}
