//! Writing a corpus for one format policy, and saying how it was made.
//!
//! A policy's corpus is meant to be *captured*: files a real tool wrote, edited the way that tool
//! edits them. This module is the capture for the one format that has no tool on hand here — PNG —
//! and it is a capture rather than a fixture because what it writes is real PNG output from a real
//! encoder, not bytes shaped *like* a PNG.
//!
//! What is not real is the editing: the versions below were made by drawing differently and saving
//! again, which is what a paint program does, but it is this file's idea of what a save looks like
//! rather than a paint program's. The score that comes out is therefore about the *format* and about
//! a plausible way of editing it, and `corpus/png-chunks/policy.toml` says so beside the numbers.
//!
//! Run it with `--capture png`; it rewrites the corpus whole, so a corpus that moved is a corpus
//! somebody made again rather than one that drifted.

// A picture is floats turned into bytes: the shares a gradient is drawn from are bounded by the side
// of the picture and are never negative, and the arithmetic lands inside a byte by construction — so
// the conversions are the picture being written rather than a loss nobody looked at.
//
// The sums are left as sums for the same sort of reason: a fused multiply-add rounds differently, and
// the corpus is committed, so the expression that draws a picture is part of what the corpus is.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops
)]

use std::fs;
use std::path::Path;

use image::{Rgba, RgbaImage};

/// The seed every picture starts from, so a corpus made twice is the same bytes twice.
const SEED: u64 = 0x5EED_1234_ABCD_0001;

/// How large the pictures are.
const SIDE: u32 = 512;

/// The eight bytes every PNG starts with, which is what a chunk is found by.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Writes the corpus for `format` under `root`, replacing whatever was there.
///
/// # Errors
///
/// Returns why the format is not one this tool can capture, and whatever writing the files fails
/// with.
pub fn corpus(root: &Path, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        "png" => png(root),
        other => {
            Err(format!("`{other}` is not a format this tool captures; it captures png").into())
        }
    }
}

/// Writes the PNG corpus under `root`, replacing whatever was there.
///
/// # Errors
///
/// Returns whatever making a directory, encoding a picture or writing a file fails with.
pub fn png(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Named for the *policy* rather than for the format: a corpus is what judges one strategy, and a
    // directory named after the format would have to be told which of the strategies for it it means.
    let policy = root.join("png-chunks");
    let _ = fs::remove_dir_all(&policy);

    // A large first version with no history: the cost of one version, and what a cut at chunk
    // boundaries does with a picture nothing has edited.
    let canvas = scene("canvas", &[]);
    write(&policy.join("big-file"), std::slice::from_ref(&canvas))?;

    // An equal-length edit: a repainted region, saved again. The pixels outside it are the same
    // pixels, but a PNG recompresses the whole picture, so this is the case a cut at chunk
    // boundaries cannot help with — which is the finding this corpus exists to record.
    write(
        &policy.join("in-place-edit"),
        &[
            canvas.clone(),
            scene("canvas", &[(0.12, 0.12, 0.30, 0.26, [220, 40, 40, 255])]),
            scene(
                "canvas",
                &[
                    (0.12, 0.12, 0.30, 0.26, [220, 40, 40, 255]),
                    (0.55, 0.60, 0.28, 0.22, [40, 90, 220, 255]),
                ],
            ),
        ],
    )?;

    // Only the metadata moved: the same pixels, a text chunk added before the end. This is the one
    // edit a cut at chunk boundaries is for, and the picture's own bytes do not move at all.
    let mut annotated = canvas.clone();
    insert_text(&mut annotated, "Comment", "this is a corpus, not a picture")?;
    write(&policy.join("metadata-only"), &[canvas.clone(), annotated])?;

    // The same bytes written again: whether a cut hits when nothing changed.
    write(
        &policy.join("full-rewrite"),
        &[canvas.clone(), canvas.clone()],
    )?;

    // One picture in two paths, each edited somewhere else: what the two still have in common is
    // deduplication that no single chain shows.
    let asset = policy.join("shared-asset");
    write(
        &asset.join("a"),
        &[
            // The last use of the canvas itself, so it is moved rather than copied again.
            canvas,
            scene("canvas", &[(0.05, 0.70, 0.20, 0.20, [30, 160, 90, 255])]),
        ],
    )?;
    write(
        &asset.join("b"),
        &[scene(
            "canvas",
            &[(0.70, 0.05, 0.22, 0.20, [200, 180, 20, 255])],
        )],
    )?;

    // What the corpus is and how it came to be, written with it so that a capture is self-contained:
    // a corpus whose provenance is a file somebody remembered to write is one that will one day
    // arrive without it.
    fs::write(policy.join("policy.toml"), PROVENANCE)?;

    Ok(())
}

/// What a captured corpus says about itself.
const PROVENANCE: &str = "\
# What this corpus is, and how it came to be.
#
# It is not captured from a paint program: there is none here. `./run.sh chunker --capture png`
# writes it, and what that writes is real PNG output from a real encoder — the `image` crate's —
# edited the way a paint program edits a picture: a region repainted, a text chunk added, the same
# bytes saved twice.
#
# So the *format* is real and the *edits* are this repository's idea of an edit. A score taken over
# these pictures is about what cutting a PNG at its chunk boundaries is worth, and about these edits;
# it is not a claim about what any particular painter's tool does to a particular picture.
#
# A corpus captured from a real tool would replace these files and keep this file's provenance
# honest — which is the point of the field below.

name = \"png-chunks\"
format = \"png\"
made-with = \"./run.sh chunker --capture png\"
patterns = [\"big-file\", \"in-place-edit\", \"metadata-only\", \"full-rewrite\", \"shared-asset\"]
";

/// Writes `versions` into `directory` as `000.png`, `001.png`, …
fn write(directory: &Path, versions: &[Vec<u8>]) -> std::io::Result<()> {
    fs::create_dir_all(directory)?;

    for (number, bytes) in versions.iter().enumerate() {
        fs::write(directory.join(format!("{number:03}.png")), bytes)?;
    }

    Ok(())
}

/// A picture of gradients with `edits` painted over it, as a PNG.
///
/// The same `label` and the same edits give the same bytes, which is what lets a corpus be made again.
/// The gradients are what makes it a picture rather than noise: a renderer's output compresses well,
/// and a corpus of noise would be a corpus no paint program produces.
fn scene(label: &str, edits: &[(f32, f32, f32, f32, [u8; 4])]) -> Vec<u8> {
    let mut noise = Noise::new(seed(label));
    let mut image = RgbaImage::new(SIDE, SIDE);

    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let across = (x as f32) / (SIDE as f32);
        let down = (y as f32) / (SIDE as f32);
        let jitter = (noise.word() % 9) as u8;

        *pixel = Rgba([
            (200.0 * across + 20.0) as u8,
            (180.0 * down + 30.0) as u8,
            (90.0 * (across + down) / 2.0 + 40.0) as u8,
            255 - jitter,
        ]);
    }

    for (left, top, width, height, colour) in edits {
        paint(&mut image, *left, *top, *width, *height, *colour);
    }

    encode(&image)
}

/// Paints a rectangle of `colour`, its bounds given as fractions of the picture.
fn paint(image: &mut RgbaImage, left: f32, top: f32, width: f32, height: f32, colour: [u8; 4]) {
    let x = (left * SIDE as f32) as u32;
    let y = (top * SIDE as f32) as u32;
    let across = ((width * SIDE as f32) as u32).min(SIDE - x);
    let down = ((height * SIDE as f32) as u32).min(SIDE - y);

    for at in x..x + across {
        for down_at in y..y + down {
            image.put_pixel(at, down_at, Rgba(colour));
        }
    }
}

/// The bytes a picture is written as.
fn encode(image: &RgbaImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("a picture writes itself");

    bytes
}

/// Inserts a `tEXt` chunk before the end of a PNG, so that only the metadata moved.
///
/// # Errors
///
/// Returns why the chunk could not be inserted, which is a file that does not read as a PNG.
fn insert_text(
    png: &mut Vec<u8>,
    keyword: &str,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut at = SIGNATURE.len();

    while at + 8 <= png.len() {
        let length = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
        let length = usize::try_from(length)?;

        if png.get(at + 4..at + 8) == Some(b"IEND") {
            let mut data = Vec::with_capacity(keyword.len() + text.len() + 1);
            data.extend_from_slice(keyword.as_bytes());
            data.push(0);
            data.extend_from_slice(text.as_bytes());

            let mut chunk = Vec::with_capacity(data.len() + 12);
            chunk.extend_from_slice(&u32::try_from(data.len())?.to_be_bytes());
            chunk.extend_from_slice(b"tEXt");
            chunk.extend_from_slice(&data);
            chunk.extend_from_slice(&crc(*b"tEXt", &data).to_be_bytes());

            png.splice(at..at, chunk);

            return Ok(());
        }

        at += 8 + length + 4;
    }

    Err("no end chunk in the picture".into())
}

/// The CRC a chunk's data is stamped with.
fn crc(kind: [u8; 4], data: &[u8]) -> u32 {
    let mut crc = u32::MAX;

    for byte in kind.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let carry = crc & 1;
            crc >>= 1;
            if carry != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }

    !crc
}

/// The seed a label stands for, so a corpus made twice is the same bytes twice.
fn seed(label: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ SEED;
    for byte in label.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    hash
}

/// A deterministic stream of words, for the small jitter a renderer would not have.
struct Noise {
    /// `splitmix64`'s counter.
    state: u64,
}

impl Noise {
    /// A stream seeded by `seed`.
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next word.
    const fn word(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);

        mixed ^ (mixed >> 31)
    }
}
