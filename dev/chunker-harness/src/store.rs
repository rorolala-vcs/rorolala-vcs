//! What a real store holds, for a chain cut with one chunker.
//!
//! The model in [`measure`](crate::measure) predicts the bytes a chain costs. This is the same chain
//! written into a store, so that the prediction can be checked rather than believed: every version is
//! written, read back and compared with the file it came from, and then the store's own `obj/` and
//! `manifest/` directories are measured.
//!
//! It is slower than the model by an order of magnitude — every byte goes through a file twice — which
//! is why it is a flag and not the default. What it buys is that the numbers in a report are the
//! store's, and the model behind them has been shown to agree.

use std::fs;
use std::path::{Path, PathBuf};

use rorolala_storage::{AlgorithmChoice, Chunking, Codec, RorolalaStorage, StorageBackend as _};

use crate::corpus::Pattern;

/// What a store ended up holding.
#[derive(Debug, Clone, Copy)]
pub struct Written {
    /// The bytes under the store's `obj/` and `manifest/`, which is everything it keeps.
    pub stored: u64,
    /// How many versions were written.
    pub versions: u64,
}

/// Writes every version of `pattern` into a store of its own, cut with `chunking`.
///
/// # Errors
///
/// Returns whatever the store, the read-back or the measurement fails with, including a version that
/// does not read back as what was written.
pub async fn measure(
    pattern: &Pattern,
    chunking: Chunking,
) -> Result<Written, Box<dyn std::error::Error>> {
    let root = scratch();
    let store = RorolalaStorage::create(&root);
    let choice = AlgorithmChoice::new(Codec::Raw, chunking);
    let mut written = Written {
        stored: 0,
        versions: 0,
    };

    for path in pattern.versions() {
        let key = store.write_file(path, choice).await?;

        // Every version is read back rather than hashed in place: a cut that does not reassemble is
        // the one failure a measurement cannot see on its own.
        let back = root.join("read-back");
        store.extract_file(&key, &back).await?;

        if fs::read(&back)? != fs::read(path)? {
            let _ = fs::remove_dir_all(&root);

            return Err(format!("{}: does not read back as it was written", path.display()).into());
        }

        fs::remove_file(&back)?;
        written.versions = written.versions.saturating_add(1);
    }

    written.stored = bytes_under(&root.join("obj"))? + bytes_under(&root.join("manifest"))?;

    fs::remove_dir_all(&root)?;

    Ok(written)
}

/// Every byte a file under `directory` takes, walked to the bottom.
fn bytes_under(directory: &Path) -> Result<u64, std::io::Error> {
    let mut total = 0_u64;

    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(total);
    };

    for entry in entries {
        let path = entry?.path();
        let metadata = fs::metadata(&path)?;

        if metadata.is_dir() {
            total = total.saturating_add(bytes_under(&path)?);
        } else {
            total = total.saturating_add(metadata.len());
        }
    }

    Ok(total)
}

/// A directory of its own, emptied first so a rerun starts clean.
///
/// A store is a place rather than a value, so two of them measured at once have to be two places.
fn scratch() -> PathBuf {
    crate::scratch::at("store")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rorolala_storage::Chunking;

    use super::measure;
    use crate::corpus::Corpus;
    use crate::generate::{Scale, synthetic};
    use crate::measure as model;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch_dir(label: &str) -> PathBuf {
        crate::scratch::at(label)
    }

    #[test]
    fn the_model_predicts_what_the_store_holds() {
        let root = scratch_dir("agree");
        let corpus = {
            synthetic(&root, "synthetic", Scale::Test).unwrap();
            Corpus::read(&root).unwrap()
        };
        let pattern = &corpus.policies[0].patterns[0];
        let runtime = tokio::runtime::Runtime::new().unwrap();

        for chunking in [
            Chunking::Whole,
            Chunking::Fixed { size: 4096 },
            Chunking::Cdc {
                min: 256,
                average: 1024,
                max: 8192,
            },
        ] {
            let predicted = model::measure(pattern, &chunking).unwrap();
            let written = runtime.block_on(measure(pattern, chunking)).unwrap();

            // Frame headers, object payloads and manifests together: the model is only worth having
            // if it is the store's own arithmetic rather than an approximation of it.
            assert_eq!(
                predicted.numbers.stored, written.stored,
                "{chunking:?}: {} predicted, {} held",
                predicted.numbers.stored, written.stored
            );
            assert_eq!(predicted.numbers.versions, written.versions);
        }
    }
}
