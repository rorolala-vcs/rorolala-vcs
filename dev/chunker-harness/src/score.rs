//! What a strategy's numbers are worth, and against what.
//!
//! A score is four ratios against the best yardstick on the same chain, combined by a weighted
//! geometric mean:
//!
//! - **dedup** — the share of the content the store did not already have. Higher is better.
//! - **worst marginal** — the largest share of one version that had to be written. Lower is better, and
//!   it is the term that says whether an edit costs a chunk or the file.
//! - **stored** — the bytes the store holds for the chain, per byte of content. Lower is better; it
//!   includes the frame in front of every object and the manifest.
//! - **manifest** — the bytes the manifests take, per byte of content. Lower is better, and it is the
//!   term that stops a strategy from "winning" by cutting into pieces too small to be worth a record.
//!
//! A geometric mean rather than a sum, because these are ratios and ratios multiply: a strategy that
//! halves the stored bytes and doubles the manifest overhead has done nothing, and an arithmetic mean
//! would still reward it.
//!
//! The weights live in `score.toml`, together with the version of the numbers and the floors a policy
//! has to clear to be called usable at all. A version bump is how a score stops being comparable with
//! one taken before it, which is the only honest thing to do when the weights move.

// A score is a ratio of byte counts; an `f64` is the right shape for it and the wrong shape for the
// bytes themselves, which are counted exactly.
#![allow(clippy::cast_precision_loss)]

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::measure::Numbers;

/// The worst a ratio is allowed to be, so that one bad term cannot make the product zero and lose the
/// rest of the comparison with it.
const MIN_RATIO: f64 = 0.01;

/// The best a ratio is allowed to be, for the same reason at the other end.
const MAX_RATIO: f64 = 100.0;

/// What one chunker's numbers say, in the four shapes the score is made of.
#[derive(Debug, Clone, Copy, Default)]
pub struct Reading {
    /// The share of the content the store did not already hold: 0 is all deduplicated, 1 is none.
    pub dedup: f64,
    /// The largest share of one version that had to be written.
    pub worst_marginal: f64,
    /// The bytes the store holds, per byte of content.
    pub stored: f64,
    /// The bytes the manifests take, per byte of content.
    pub manifest: f64,
}

/// One chunker's score, and the ratios it was made of.
#[derive(Debug, Clone, Copy)]
pub struct Scored {
    /// The weighted geometric mean of the ratios. 1.0 is "level with the best yardstick here".
    pub total: f64,
    /// The four ratios, for a reader who wants to know why.
    pub ratios: Reading,
}

/// What a policy's numbers are worth.
///
/// `dedup` is reported as what was *kept* — the whole content minus what the store already had — so
/// every term of the score is one where less is worse, and a reader does not have to hold two
/// directions in their head at once.
#[must_use]
pub fn reading(numbers: &Numbers) -> Reading {
    Reading {
        dedup: 1.0 - numbers.undeduped(),
        worst_marginal: numbers.worst_marginal,
        stored: numbers.stored_share(),
        manifest: numbers.manifest_share(),
    }
}

/// The file the score is configured by.
///
/// Read rather than compiled in: what a term should weigh is an argument a reader should be able to
/// have with the numbers, and an argument made by editing a file is one that leaves a diff.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weights {
    /// The version of these numbers. A score is only comparable with one taken under the same one.
    pub version: u32,
    /// What a policy has to clear to be called usable at all.
    pub floors: Floors,
    /// What each term weighs.
    pub terms: Terms,
}

/// The floors a chunker has to clear to be usable, whatever its score.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Floors {
    /// How fast a cut has to be, in megabytes a second, over the whole corpus.
    pub min_split_mb_per_s: f64,
    /// The largest a cut's working set may be, as a multiple of the version it cut.
    pub max_peak_multiple: f64,
}

/// What each term of the score weighs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Terms {
    /// The weight of the deduplication term.
    pub dedup: f64,
    /// The weight of the worst-marginal-version term.
    pub worst_marginal: f64,
    /// The weight of the stored-bytes term.
    pub stored: f64,
    /// The weight of the manifest term.
    pub manifest: f64,
}

impl Weights {
    /// Reads the weights from `path`.
    ///
    /// # Errors
    ///
    /// Returns whatever reading or parsing the file fails with. A field this build does not know is an
    /// error rather than ignored: a weight that is written but not read would leave the score meaning
    /// something other than the file says.
    pub fn read(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let text = fs::read_to_string(path)?;

        Ok(toml::from_str(&text)?)
    }

    /// The sum of the weights, which is what the geometric mean divides by.
    fn weight(&self) -> f64 {
        self.terms.dedup + self.terms.worst_marginal + self.terms.stored + self.terms.manifest
    }
}

impl Floors {
    /// Whether `numbers` cuts fast enough.
    #[must_use]
    pub fn speed(&self, numbers: &Numbers) -> bool {
        numbers.speed() >= self.min_split_mb_per_s
    }

    /// Whether `numbers` cuts within the memory it is allowed.
    #[must_use]
    pub fn memory(&self, numbers: &Numbers) -> bool {
        numbers.worst_peak <= self.max_peak_multiple
    }
}

/// Scores every reading against the best of them, term by term.
///
/// The best of each term is taken over the readings given, so a table is always scored against itself:
/// adding a strategy can lower everyone's ratio, which is what makes the table a yardstick rather than
/// a set of independent opinions.
#[must_use]
pub fn score_all(readings: &[Reading], weights: &Weights) -> Vec<Scored> {
    let best = |term: fn(&Reading) -> f64, higher: bool| -> f64 {
        let mut best = if higher {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };

        for reading in readings {
            let value = term(reading);
            best = if higher {
                best.max(value)
            } else {
                best.min(value)
            };
        }

        best
    };

    let dedup = best(|reading| reading.dedup, true);
    let marginal = best(|reading| reading.worst_marginal, false);
    let stored = best(|reading| reading.stored, false);
    let manifest = cheapest_manifest(readings);

    readings
        .iter()
        .map(|reading| {
            let ratios = Reading {
                dedup: higher(reading.dedup, dedup),
                worst_marginal: lower(reading.worst_marginal, marginal),
                stored: lower(reading.stored, stored),
                // The term is the price of cutting, and a chunker that does not cut pays nothing:
                // its share of zero is neutral rather than the best. Making zero the best would
                // score every cutting chunker against a mark it cannot reach, and the term would
                // stop comparing them and become a constant penalty on all of them.
                manifest: if reading.manifest <= 0.0 {
                    1.0
                } else {
                    lower(reading.manifest, manifest)
                },
            };

            Scored {
                total: mean(&ratios, weights),
                ratios,
            }
        })
        .collect()
}

/// The smallest non-zero manifest share among the readings.
///
/// A reading that writes no manifest at all is left out rather than counted as zero; when none of
/// them writes one, every ratio in the term is 1 and the term says nothing.
fn cheapest_manifest(readings: &[Reading]) -> f64 {
    let mut cheapest = f64::INFINITY;

    for reading in readings {
        if reading.manifest > 0.0 {
            cheapest = cheapest.min(reading.manifest);
        }
    }

    if cheapest.is_finite() { cheapest } else { 1.0 }
}

/// The weighted geometric mean of the ratios.
// A sum of four products, written as a sum: `mul_add` would make it one expression nobody can read,
// and what the last bit of a score's precision is worth here is nothing next to being able to check it.
#[allow(clippy::suboptimal_flops)]
fn mean(ratios: &Reading, weights: &Weights) -> f64 {
    let weight = weights.weight();
    if weight <= 0.0 {
        return 1.0;
    }

    let terms = &weights.terms;
    let weighted = terms.dedup * ratios.dedup.ln()
        + terms.worst_marginal * ratios.worst_marginal.ln()
        + terms.stored * ratios.stored.ln()
        + terms.manifest * ratios.manifest.ln();

    (weighted / weight).exp()
}

/// A ratio where more is better.
fn higher(value: f64, best: f64) -> f64 {
    if best <= 0.0 {
        return if value <= 0.0 { 1.0 } else { MAX_RATIO };
    }

    (value / best).clamp(MIN_RATIO, MAX_RATIO)
}

/// A ratio where less is better.
fn lower(value: f64, best: f64) -> f64 {
    if value <= 0.0 {
        return if best <= 0.0 { 1.0 } else { MAX_RATIO };
    }

    if best <= 0.0 {
        return MIN_RATIO;
    }

    (best / value).clamp(MIN_RATIO, MAX_RATIO)
}

#[cfg(test)]
mod tests {
    use super::{Floors, Reading, Terms, Weights, score_all};

    /// Weights where every term counts the same, so the score is the plain geometric mean.
    fn even() -> Weights {
        Weights {
            version: 1,
            floors: Floors {
                min_split_mb_per_s: 0.0,
                max_peak_multiple: f64::MAX,
            },
            terms: Terms {
                dedup: 1.0,
                worst_marginal: 1.0,
                stored: 1.0,
                manifest: 1.0,
            },
        }
    }

    #[test]
    fn the_best_of_every_term_scores_one() {
        let best = Reading {
            dedup: 0.9,
            worst_marginal: 0.1,
            stored: 0.2,
            manifest: 0.01,
        };
        let worse = Reading {
            dedup: 0.5,
            worst_marginal: 0.4,
            stored: 0.8,
            manifest: 0.04,
        };

        let scored = score_all(&[best, worse], &even());

        assert!((scored[0].total - 1.0).abs() < 1e-9, "{}", scored[0].total);
        assert!(scored[1].total < 1.0, "{}", scored[1].total);
    }

    #[test]
    fn the_score_is_the_geometric_mean_of_its_ratios() {
        // Equal weights make the score the plain fourth root of the four ratios, which is the property
        // the whole scheme rests on: a term that doubles lowers the score by 2^(1/4) and four of them
        // halve it, whatever else the others do.
        let rows = [
            Reading {
                dedup: 0.8,
                worst_marginal: 0.2,
                stored: 0.4,
                manifest: 0.02,
            },
            Reading {
                dedup: 0.4,
                worst_marginal: 0.1,
                stored: 0.4,
                manifest: 0.02,
            },
        ];

        let scored = score_all(&rows, &even());

        for one in &scored {
            let ratios = one.ratios;
            let expected =
                (ratios.dedup * ratios.worst_marginal * ratios.stored * ratios.manifest).powf(0.25);

            assert!(
                (one.total - expected).abs() < 1e-9,
                "{} != {expected}",
                one.total
            );
            assert!(one.total <= 1.0 + 1e-9, "{}", one.total);
        }

        // Each of the two is twice as bad as the other at two of the four terms, so neither wins: the
        // score is where the two of them meet, which is what a geometric mean does and a sum does not.
        assert!((scored[0].total - scored[1].total).abs() < 1e-9);
    }

    #[test]
    fn a_heavier_weight_pushes_the_weaker_row_further_down() {
        let strong = Reading {
            dedup: 0.8,
            worst_marginal: 0.2,
            stored: 0.4,
            manifest: 0.02,
        };
        // Behind on deduplication and level everywhere else, so only one term can move the score.
        let weak = Reading {
            dedup: 0.4,
            worst_marginal: 0.2,
            stored: 0.4,
            manifest: 0.02,
        };

        let mut weights = even();
        let plain = score_all(&[strong, weak], &weights)[1].total;

        weights.terms.dedup = 8.0;
        let heavier = score_all(&[strong, weak], &weights)[1].total;

        // Weighting a term more is not decoration: a row that is behind on it has to fall.
        assert!(heavier < plain, "{heavier} !< {plain}");
    }

    #[test]
    fn a_reading_with_no_manifest_at_all_is_not_infinite() {
        // The `whole` chunker writes no manifest, so the term's best is zero and every other reading
        // has to divide by it. A score that came out infinite would poison the table it is printed in.
        let whole = Reading {
            dedup: 0.0,
            worst_marginal: 1.0,
            stored: 1.0,
            manifest: 0.0,
        };
        let cut = Reading {
            dedup: 0.5,
            worst_marginal: 0.2,
            stored: 0.6,
            manifest: 0.03,
        };

        let scored = score_all(&[whole, cut], &even());

        assert!(scored.iter().all(|one| one.total.is_finite()));
        assert!((scored[0].ratios.manifest - 1.0).abs() < 1e-9);
        assert!((scored[1].ratios.stored - 1.0).abs() < 1e-9);
    }
}
