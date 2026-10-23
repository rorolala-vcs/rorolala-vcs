//! Draws the icons the tree's descriptions under `res/icons` ask for.
//!
//! It is a build script of its own rather than part of the workspace's because what it makes is read
//! by another build script: `rola` and `rola-daemon` link `res/icons/rola-cli.ico` in as their
//! Windows resource, and cargo may run either build script before the workspace's. A build
//! dependency is the ordering cargo does promise — its build script runs before the build script of
//! anything that depends on it — so the workspace and the two programs all depend on this crate, and
//! a tree that was just checked out has its icons before anything looks for one.
//!
//! A description sits beside the image it describes, named `<image>.<extension>.toml`, and says what
//! to make of it: the format, the size, and how the source is sampled down to that size. What it
//! makes is written beside both and is deliberately not kept in the repository — see
//! `res/icons/.gitignore` — so this is where a fresh tree gets it.

use std::path::{Path, PathBuf};

use image::ImageEncoder;
use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::FilterType;

/// Directory, from the workspace root, of the icons and the descriptions that make them.
const ICON_DIR: &str = "res/icons";

/// The target format an icon description may ask for.
///
/// Only the one is understood. Another is an error rather than something skipped: a description
/// nothing acts on is a description whose author believes it did.
const ICON_FORMAT: &str = "ico";

/// The scaling that takes the nearest source pixel.
const SCALING_NEAREST: &str = "nearest";

/// The scaling that reads the whole of the source into every target pixel.
const SCALING_LANCZOS3: &str = "lanczos3";

/// The largest side an ICO frame may have, which is what the format's directory can state.
const ICO_MAX_SIDE: u32 = 256;

fn main() {
    generate_icons();
}

/// The workspace root, which is two directories above this crate.
fn manifest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the workspace root is two directories above this crate")
        .to_owned()
}
/// Makes the icons the descriptions under `res/icons` ask for, beside them.
///
/// A description sits beside the image it describes, named `<image>.<extension>.toml`, and says what
/// to make of it: the format, the size, and how the source is sampled down to that size. What it
/// makes is written beside both and is deliberately not kept in the repository — see
/// `res/icons/.gitignore` — so this is where a fresh tree gets it.
///
/// A description may instead name several sources with `sources`, which makes one multi-frame ICO
/// rather than a single-frame one: the desktop program and the installer are drawn at whatever size
/// a place asks for, and the frames are what answer those sizes. The frames are the sources at their
/// own sizes, so `target-size` is refused beside `sources` and `scaling` has nothing to sample.
///
/// The whole of `res/icons` is walked, so a family of pictures may be kept in a directory of its own
/// rather than flattened among every other description. What a description names is read relative to
/// the description itself, so a source beside it needs no path and one elsewhere is written out.
///
/// A description that cannot be acted on fails the build rather than being skipped. What a generated
/// file is for is a description that was carried out, so one that was not is one that would go
/// unnoticed until something looked for the file and did not find it.
///
/// Nothing here runs when only the .NET side is built: `dotnet build` reads the generated files, and
/// reaching them means the workspace has been built first.
fn generate_icons() {
    // Watched as a directory rather than file by file, so that a description being added or removed
    // is noticed without this list having to be kept in step by hand.
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir().join(ICON_DIR).display()
    );

    let dir = manifest_dir().join(ICON_DIR);

    let mut descriptions = Vec::new();
    descriptions_under(&dir, &mut descriptions);

    // Sorted so that the same tree makes the same files in the same order, whatever the filesystem
    // hands back: a build script that acts in the order it happens to read is not reproducible. The
    // walk gathers in whatever order a directory lists, so the order is decided here instead.
    descriptions.sort();

    let mut failed = false;

    for description in descriptions {
        if let Err(error) = generate_icon(&description) {
            println!("cargo:warning={error}");
            failed = true;
        }
    }

    if failed {
        std::process::exit(1);
    }
}

/// Collects every description under a directory, at any depth.
///
/// A directory that cannot be read is passed over rather than failing: this is also the first call,
/// on the whole of `res/icons`, and a tree without one has nothing for this to do — which is not an
/// error, since an icon that is generated is generated by a description that is there.
fn descriptions_under(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            descriptions_under(&path, found);
            continue;
        }

        if path
            .extension()
            .is_some_and(|extension| extension == "toml")
        {
            found.push(path);
        }
    }
}

/// Makes the one icon a description asks for.
///
/// The error is the line to print rather than a type: what a caller does with it is say it, and one
/// line is the whole of what a build script can say.
///
/// # Errors
///
/// Fails when the description cannot be read or does not state what it has to, when the source
/// picture cannot be read, or when the icon cannot be written.
fn generate_icon(description: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(description)
        .map_err(|error| format!("{}: {error}", description.display()))?;

    let stated: toml::Value = text
        .parse()
        .map_err(|error| format!("{}: {error}", description.display()))?;

    let format = text_setting(&stated, description, "target-format")?;

    if format != ICON_FORMAT {
        return Err(format!(
            "{}: target-format is `{format}`, and `{ICON_FORMAT}` is the only one understood",
            description.display()
        ));
    }

    // Checked before the scaling and the size, because a description naming sources is a different
    // shape of description rather than the one below with one field changed: it has no single source
    // to read a size from, and no size to sample to.
    if let Some(sources) = sources_setting(&stated, description)? {
        return generate_frames(description, &stated, &sources);
    }

    let filter = match text_setting(&stated, description, "scaling")?.as_str() {
        SCALING_NEAREST => FilterType::Nearest,
        SCALING_LANCZOS3 => FilterType::Lanczos3,
        other => {
            return Err(format!(
                "{}: scaling is `{other}`, and `{SCALING_NEAREST}` and `{SCALING_LANCZOS3}` are the ones understood",
                description.display()
            ));
        }
    };

    let requested = size_setting(&stated, description)?;

    // A description is named after what it describes, so the source is its own name without the
    // `.toml`, and what is made of the source is that name under the target format.
    let source = description.with_extension("");
    let output = source.with_extension(ICON_FORMAT);

    let picture = image::open(&source)
        .map_err(|error| format!("{}: {error}", source.display()))?
        .into_rgba8();

    // Worked out from the picture rather than from the description alone, because a size that leaves
    // a side out asks for the shape this picture has, which is not something the description can say.
    let (width, height) = requested.resolve((picture.width(), picture.height()), description)?;

    let resized = image::imageops::resize(&picture, width, height, filter);

    let mut encoded = Vec::new();
    IcoEncoder::new(&mut encoded)
        .write_image(
            resized.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("{}: {error}", output.display()))?;

    write_if_different(&output, &encoded)
}

/// Makes the one multi-frame icon a description naming several sources asks for.
///
/// Each source is a frame at its own size, in the order the description names them: what a frame is
/// for is one of the sizes a place may draw the picture at, and a frame resized to another size
/// would be a second copy of one the picture already has. The description therefore states no size —
/// the sources are the sizes — and `scaling` is read but not used, because nothing is sampled.
///
/// # Errors
///
/// Fails when a source cannot be read, is larger than an ICO frame may be, or when the icon cannot
/// be written.
fn generate_frames(
    description: &Path,
    stated: &toml::Value,
    sources: &[String],
) -> Result<(), String> {
    // Stated but not acted on, so it is read for the error it would have been: a description that
    // says how to sample and is never sampled is a description its author believes more of than the
    // build does.
    if let Some(size) = stated.get("target-size").and_then(toml::Value::as_str) {
        return Err(format!(
            "{}: target-size is `{size}`, and a description naming sources takes each frame at its own size",
            description.display()
        ));
    }

    if let Some(scaling) = stated.get("scaling").and_then(toml::Value::as_str)
        && scaling != SCALING_NEAREST
        && scaling != SCALING_LANCZOS3
    {
        return Err(format!(
            "{}: scaling is `{scaling}`, and `{SCALING_NEAREST}` and `{SCALING_LANCZOS3}` are the ones understood",
            description.display()
        ));
    }

    // The sources are named relative to the description that names them, which is where they sit.
    let directory = description.parent().unwrap_or_else(|| Path::new("."));

    let mut frames = Vec::with_capacity(sources.len());

    for named in sources {
        let source = directory.join(named);

        let picture = image::open(&source)
            .map_err(|error| format!("{}: {error}", source.display()))?
            .into_rgba8();

        let (width, height) = (picture.width(), picture.height());

        // The directory of an ICO states a side in one byte, so 256 is written as 0 and 257 has no
        // spelling at all: a larger source is refused rather than sampled down, because sampling is
        // what the frame is there to avoid.
        let limit = ICO_MAX_SIDE;

        if width == 0 || height == 0 || width > limit || height > limit {
            return Err(format!(
                "{}: is `{width}x{height}`, and an ICO frame is between 1 and {ICO_MAX_SIDE} on a side",
                source.display()
            ));
        }

        frames.push(
            IcoFrame::as_png(
                picture.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|error| format!("{}: {error}", source.display()))?,
        );
    }

    let mut encoded = Vec::new();
    IcoEncoder::new(&mut encoded)
        .encode_images(&frames)
        .map_err(|error| format!("{}: {error}", description.display()))?;

    write_if_different(&description.with_extension(ICON_FORMAT), &encoded)
}

/// Writes the icon only when what is there differs from what was made.
///
/// This directory is watched, so writing the same bytes again would be a build script that changes
/// its own input and so runs again on every build after it.
///
/// # Errors
///
/// Fails when the icon cannot be written.
fn write_if_different(output: &Path, encoded: &[u8]) -> Result<(), String> {
    if std::fs::read(output).is_ok_and(|written| written == encoded) {
        return Ok(());
    }

    std::fs::write(output, encoded).map_err(|error| format!("{}: {error}", output.display()))
}

/// Reads the sources a description names, if it names any.
///
/// # Errors
///
/// Fails when `sources` is stated as something other than a list of text.
fn sources_setting(
    stated: &toml::Value,
    description: &Path,
) -> Result<Option<Vec<String>>, String> {
    let Some(value) = stated.get("sources") else {
        return Ok(None);
    };

    let Some(sources) = value.as_array() else {
        return Err(format!(
            "{}: sources is not a list of pictures",
            description.display()
        ));
    };

    let mut named = Vec::with_capacity(sources.len());

    for source in sources {
        let Some(text) = source.as_str() else {
            return Err(format!(
                "{}: a source is not stated as text",
                description.display()
            ));
        };

        named.push(text.to_owned());
    }

    if named.is_empty() {
        return Err(format!(
            "{}: sources names no picture, and an ICO holds at least one",
            description.display()
        ));
    }

    Ok(Some(named))
}

/// Reads one setting a description states as text.
///
/// # Errors
///
/// Fails when the setting is not stated, or is not stated as text.
fn text_setting(stated: &toml::Value, description: &Path, key: &str) -> Result<String, String> {
    stated
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{}: {key} is not stated as text", description.display()))
}

/// The size a description asks for.
///
/// A side that is left out is what keeps the picture's shape: a description saying `256x` states a
/// width and leaves the height to the picture, which is what a caller who knows one dimension of the
/// place the picture goes wants to say. That is also why a size is not two numbers here — the one it
/// leaves out is only known once the picture it describes has been read.
enum TargetSize {
    /// Both sides are stated.
    Both(u32, u32),

    /// The width is stated and the height follows the picture.
    Width(u32),

    /// The height is stated and the width follows the picture.
    Height(u32),
}

impl TargetSize {
    /// Works out the two sides, the one that was left out following the source's shape.
    ///
    /// # Errors
    ///
    /// Fails when the sides do not fit an ICO frame — either because the description asked for a size
    /// it cannot have, or because following this picture's shape worked out to one.
    fn resolve(self, source: (u32, u32), description: &Path) -> Result<(u32, u32), String> {
        // Worked out in 64 bits, so that following a shape cannot overflow before it is known to be
        // too large for an icon to be drawn at anyway.
        let (width, height) = match self {
            Self::Both(width, height) => (u64::from(width), u64::from(height)),
            Self::Width(width) => (u64::from(width), following(width, source.0, source.1)),
            Self::Height(height) => (following(height, source.1, source.0), u64::from(height)),
        };

        let limit = u64::from(ICO_MAX_SIDE);

        if width == 0 || height == 0 || width > limit || height > limit {
            return Err(format!(
                "{}: target-size works out to `{width}x{height}`, and an ICO frame is between 1 and {ICO_MAX_SIDE} on a side",
                description.display()
            ));
        }

        Ok((
            u32::try_from(width).expect("a side within the limit is a u32"),
            u32::try_from(height).expect("a side within the limit is a u32"),
        ))
    }
}

/// Works out the side that was left out: `side` scaled by the shape `known` and `other` describe.
///
/// Rounded to the nearest whole pixel rather than truncated, and never down to nothing: a side of
/// zero is a picture with no picture in it, and one pixel is the least a side can be.
fn following(side: u32, known: u32, other: u32) -> u64 {
    if known == 0 {
        return u64::from(side);
    }

    let scaled = (u64::from(side) * u64::from(other) + u64::from(known) / 2) / u64::from(known);

    scaled.max(1)
}

/// Reads the size a description asks for.
///
/// # Errors
///
/// Fails when the size is not stated as text, is not written `WxH`, `Wx` or `xH`, states neither
/// side, or states a side that is not a number.
fn size_setting(stated: &toml::Value, description: &Path) -> Result<TargetSize, String> {
    let size = text_setting(stated, description, "target-size")?;

    let Some((width, height)) = size.split_once('x') else {
        return Err(format!(
            "{}: target-size is `{size}`, and a size is written `WxH`, `Wx` or `xH`",
            description.display()
        ));
    };

    let side = |value: &str, name: &str| -> Result<Option<u32>, String> {
        if value.is_empty() {
            return Ok(None);
        }

        value.parse::<u32>().map(Some).map_err(|error| {
            format!(
                "{}: target-size is `{size}`, whose {name} `{value}` is not a number: {error}",
                description.display()
            )
        })
    };

    match (side(width, "width")?, side(height, "height")?) {
        (Some(width), Some(height)) => Ok(TargetSize::Both(width, height)),
        (Some(width), None) => Ok(TargetSize::Width(width)),
        (None, Some(height)) => Ok(TargetSize::Height(height)),
        (None, None) => Err(format!(
            "{}: target-size is `{size}`, which states neither side",
            description.display()
        )),
    }
}
