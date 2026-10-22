//! The frozen yardsticks a strategy is measured against.
//!
//! These are the cuts the store falls back on: the ones it already had, unchanged and untuned. They
//! are not selectable by name from the command line and they are not policies — a store reaches them
//! through `/extension/nothing claims it`, not through a name. What makes them the yardsticks is
//! exactly that: their numbers do not move when a strategy is written, so a score taken this year can
//! be compared with one taken in three.
//!
//! The sizes are the ones a store's own constants use where there is one — `text` is
//! `Chunking::Text { min: 1024, average: 4096, max: 16384 }`, because that is what a text write
//! actually gets — and round powers of two elsewhere, so a reader can see what was measured.

use rorolala_storage::{Cdc, Chunking};

/// One yardstick.
#[derive(Debug, Clone, Copy)]
pub struct Baseline {
    /// What the report calls it.
    pub name: &'static str,
    /// What it cuts with.
    pub chunking: Chunking,
}

/// Every yardstick, in the order the report lists them.
#[must_use]
pub fn all() -> Vec<Baseline> {
    vec![
        Baseline {
            name: "whole",
            chunking: Chunking::Whole,
        },
        Baseline {
            name: "fixed-4k",
            chunking: Chunking::Fixed { size: 4096 },
        },
        Baseline {
            name: "fixed-64k",
            chunking: Chunking::Fixed { size: 65536 },
        },
        Baseline {
            name: "cdc-1k",
            chunking: chunking_of(Cdc::new(256, 1024, 8192)),
        },
        Baseline {
            name: "cdc-4k",
            chunking: chunking_of(Cdc::new(1024, 4096, 32768)),
        },
        Baseline {
            name: "cdc-16k",
            chunking: chunking_of(Cdc::new(4096, 16384, 131_072)),
        },
        Baseline {
            name: "cdc-64k",
            chunking: chunking_of(Cdc::new(16384, 65536, 524_288)),
        },
        // The bounds a text write gets, spelled as the store spells them, in `consts.rs`.
        Baseline {
            name: "text",
            chunking: Chunking::Text {
                min: 1024,
                average: 4096,
                max: 16384,
            },
        },
        Baseline {
            name: "zip",
            chunking: Chunking::Zip,
        },
    ]
}

/// The `Chunking` value that cuts the way `cdc` does.
///
/// The two are the same cut said two ways — `Cdc` is the doing and `Chunking::Cdc` is the choice — and
/// the store is written in terms of the choice, so this is the spelling that measures what a store
/// would do.
const fn chunking_of(cdc: Cdc) -> Chunking {
    Chunking::Cdc {
        min: cdc.min(),
        average: cdc.average(),
        max: cdc.max(),
    }
}

#[cfg(test)]
mod tests {
    use super::all;

    #[test]
    fn every_yardstick_has_a_name_of_its_own() {
        let mut names: Vec<&str> = all().iter().map(|baseline| baseline.name).collect();
        let count = names.len();

        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), count);
        assert!(count >= 5);
    }
}
