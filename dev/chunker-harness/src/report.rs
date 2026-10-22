//! What the numbers are, printed.
//!
//! A report is a diff against the yardsticks, per pattern and then over the whole policy, in that
//! order: a strategy that is good on one pattern and ruinous on another is a strategy that has not
//! been understood, and a single number at the top hides exactly that.
//!
//! What is printed is the ratios rather than the raw bytes, because the ratios are what a reader acts
//! on: "8% behind `cdc-16k` on deduplication" is a thing to go and fix, while "the store holds
//! 9,412,201 bytes" is not.

use std::collections::BTreeMap;
use std::fs;

use crate::baseline;
use crate::corpus::{Corpus, Pattern, Policy};
use crate::measure::{self, Measured};
use crate::score::{self, Reading, Weights};
use crate::store;

/// The width the chunker's name is printed in.
const NAME: usize = 12;

/// The rate above which a printed speed stops being a measurement.
///
/// A cut that does no memory traffic — by position, or not at all — is limited by its own allocations
/// rather than by the content, so the number it produces describes an allocator and not a scan. Above
/// this, the column says so instead of printing a figure nobody can compare with anything.
const TOO_FAST: f64 = 10_000.0;

/// The file that makes a corpus directory a policy's rather than a chain set.
const POLICY_FILE: &str = "policy.toml";

/// The record a policy's corpus carries: what it scores, and against what.
const SCORE_FILE: &str = "SCORE.md";

/// Prints the report for every policy in the corpus.
///
/// A corpus directory with a `policy.toml` beside its patterns belongs to a policy, and the policy has
/// to be one this build carries: a corpus for a name nothing answers to is a typo, and measuring the
/// baselines over it would look like a measurement of something. A directory with no `policy.toml` is
/// a chain set and nothing more — the synthetic one — and is measured against the baselines alone.
///
/// # Errors
///
/// Returns whatever reading a version, measuring it, or checking it against a store fails with.
pub async fn print(
    corpus: &Corpus,
    weights: &Weights,
    through_store: bool,
    scoring: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    for policy in &corpus.policies {
        let hash = corpus_hash(policy)?;
        let own = own_policy(policy)?;

        println!();
        println!(
            "corpus {}   {} pattern{}   weights v{}   content {}",
            policy.name,
            policy.patterns.len(),
            if policy.patterns.len() == 1 { "" } else { "s" },
            weights.version,
            hash,
        );
        println!(
            "  floors: split >= {:.1} MB/s, memory <= {:.1}x",
            weights.floors.min_split_mb_per_s, weights.floors.max_peak_multiple
        );

        let mut aggregate: BTreeMap<&'static str, Measured> = BTreeMap::new();

        for pattern in &policy.patterns {
            let mut rows: Vec<(&'static str, Measured)> = Vec::new();
            let mut chunkings: Vec<(&'static str, rorolala_storage::Chunking)> = Vec::new();

            for baseline in baseline::all() {
                let measured = measure::measure(pattern, &baseline.chunking)?;
                rows.push((baseline.name, measured));
                chunkings.push((baseline.name, baseline.chunking));
            }

            // The policy itself, when the corpus belongs to one, measured over the same bytes and
            // ranked with everything else: a score that excluded it would not be a comparison.
            if let Some(own) = own {
                let chunking = rorolala_storage::Chunking::Policy(own.name);
                rows.push((own.name, measure::measure(pattern, &chunking)?));
                chunkings.push((own.name, chunking));
            }

            println!(
                "\npattern {:<16} {} version{}, {} bytes",
                pattern.name,
                pattern.len(),
                if pattern.len() == 1 { "" } else { "s" },
                total_bytes(pattern)?,
            );
            print_rows(&rows, weights);

            if through_store {
                check_against_store(pattern, &chunkings, &rows).await?;
            }

            for (name, measured) in rows {
                match aggregate.get_mut(name) {
                    Some(at) => at.merge(&measured),
                    None => {
                        aggregate.insert(name, measured);
                    }
                }
            }
        }

        println!(
            "\npolicy {:<16} {} pattern{}, {} versions",
            policy.name,
            policy.patterns.len(),
            if policy.patterns.len() == 1 { "" } else { "s" },
            aggregate
                .values()
                .map(|one| one.numbers.versions)
                .max()
                .unwrap_or(0),
        );

        let merged: Vec<(&'static str, Measured)> = aggregate.into_iter().collect();
        print_rows(&merged, weights);

        if scoring && own.is_some() {
            write_score(policy, &merged, weights, &hash)?;
        }
    }

    Ok(())
}

/// The policy a corpus belongs to, when it belongs to one.
///
/// A directory with a `policy.toml` beside its patterns is a policy's corpus, and the policy has to be
/// one this build carries: a corpus for a name nothing answers to is a typo, and a run that measured
/// only the baselines over it would look like a measurement of something that is not there.
fn own_policy(
    policy: &Policy,
) -> Result<Option<&'static rorolala_storage::PolicyEntry>, Box<dyn std::error::Error>> {
    if !policy.directory.join(POLICY_FILE).is_file() {
        return Ok(None);
    }

    rorolala_storage::policy(&policy.name).map_or_else(
        || {
            Err(format!(
                "{}: a corpus for `{}`, which this build does not carry — it has {}",
                policy.directory.display(),
                policy.name,
                match rorolala_storage::names().count() {
                    0 => "none".to_owned(),
                    1 => rorolala_storage::names().next().unwrap_or("").to_owned(),
                    _ => rorolala_storage::names().collect::<Vec<_>>().join(", "),
                }
            )
            .into())
        },
        |own| Ok(Some(own)),
    )
}

/// Writes `SCORE.md` beside a policy's corpus, from the numbers just measured.
///
/// It is generated and committed rather than written by hand, which is the whole of what makes it a
/// record: a number nobody re-derives is a number that stops being true, and the gate re-runs this and
/// asks git whether the file moved.
fn write_score(
    policy: &Policy,
    rows: &[(&'static str, Measured)],
    weights: &Weights,
    hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fmt::Write as _;

    let passing: Vec<Reading> = rows
        .iter()
        .filter(|(_, measured)| measured.gates.ok())
        .map(|(_, measured)| score::reading(&measured.numbers))
        .collect();
    let totals = score::score_all(&passing, weights);

    // Back from the scores to the names, skipping the rows the gates refused.
    let mut at = 0;
    let mut own = None;
    let mut best: Option<(&'static str, f64)> = None;
    for (name, measured) in rows {
        if !measured.gates.ok() {
            continue;
        }

        let total = totals[at].total;
        at += 1;

        if *name == policy.name {
            own = Some(total);
        } else if best.is_none_or(|(_, had)| total > had) {
            best = Some((name, total));
        }
    }

    let versions = rows
        .iter()
        .map(|(_, measured)| measured.numbers.versions)
        .max()
        .unwrap_or(0);
    let claims = rorolala_storage::policy(&policy.name)
        .map(|own| own.extensions)
        .unwrap_or_default();

    let mut text = String::new();
    let _ = writeln!(text, "# `{}` — what it scores", policy.name);
    let _ = writeln!(
        text,
        "\nGenerated by `./run.sh chunker --score`. Do not edit.\n"
    );
    let _ = writeln!(text, "| | |");
    let _ = writeln!(text, "| --- | --- |");
    let _ = writeln!(text, "| corpus | `{hash}` |");
    let _ = writeln!(
        text,
        "| patterns | {} |",
        policy
            .patterns
            .iter()
            .map(|pattern| pattern.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(text, "| versions | {versions} |");
    let _ = writeln!(text, "| weights | v{} |", weights.version);
    let _ = writeln!(
        text,
        "| claims | {} |",
        if claims.is_empty() {
            "no container".to_owned()
        } else {
            claims
                .iter()
                .map(|claim| format!("`.{claim}`"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    let _ = writeln!(text, "\n```\n{}```", table(rows, weights, false));

    match (own, best) {
        (Some(own), Some((name, best))) => {
            let _ = writeln!(
                text,
                "\nIt scores **{own:.3}** against **{best:.3}** for the best general chunker here, `{name}`."
            );
            if own >= best {
                let _ = writeln!(text, "It is at or ahead of it.");
            } else {
                let _ = writeln!(
                    text,
                    "It is {:.1}% behind it, and a strategy behind the general cut is not one to \
                     give a container to.",
                    (1.0 - own / best) * 100.0
                );
            }
        }
        _ => {
            let _ = writeln!(
                text,
                "\nThe policy is not in the table: its cut failed a gate, and a cut that does not \
                 reassemble has no score."
            );
        }
    }

    std::fs::write(policy.directory.join(SCORE_FILE), text)?;

    Ok(())
}

/// Prints one table of chunkers, best score first.
fn print_rows(rows: &[(&'static str, Measured)], weights: &Weights) {
    print!("{}", table(rows, weights, true));
}

/// One table of chunkers, best score first, as text.
///
/// `timing` is whether the rate column is in it. A committed `SCORE.md` leaves it out: a rate is the
/// one number here that depends on the machine it was taken on, and a record the gate compares byte
/// for byte cannot carry one.
fn table(rows: &[(&'static str, Measured)], weights: &Weights, timing: bool) -> String {
    use std::fmt::Write as _;

    let passing: Vec<Reading> = rows
        .iter()
        .filter(|(_, measured)| measured.gates.ok())
        .map(|(_, measured)| score::reading(&measured.numbers))
        .collect();
    let scored = score::score_all(&passing, weights);

    // A chunker whose cut failed has no score to rank: it is put after every one that has, and says
    // why. What the gates are for is that no number from it reaches the table at all.
    let mut at = 0;
    let mut order: Vec<(f64, f64, &'static str, &Measured)> = Vec::with_capacity(rows.len());
    for (name, measured) in rows {
        let (total, weakest) = if measured.gates.ok() {
            let one = scored[at];
            at += 1;

            // The weakest of the four ratios, which is the one to go and fix: a strategy is only as
            // good as the term it is furthest behind on.
            let ratios = one.ratios;
            let weakest = ratios
                .dedup
                .min(ratios.worst_marginal)
                .min(ratios.stored)
                .min(ratios.manifest);

            (one.total, weakest)
        } else {
            (f64::NEG_INFINITY, f64::NEG_INFINITY)
        };

        order.push((total, weakest, name, measured));
    }
    order.sort_by(|left, right| right.0.total_cmp(&left.0));

    let mut text = String::new();
    let mut header = vec![
        format!("  {:<NAME$}", "chunker"),
        format!("{:>7}", "score"),
        format!("{:>7}", "worst"),
        format!("{:>7}", "dedup"),
        format!("{:>9}", "marginal"),
        format!("{:>8}", "stored"),
        format!("{:>9}", "manifest"),
        format!("{:>8}", "chunks"),
    ];
    if timing {
        header.push(format!("{:>9}", "MB/s"));
    }
    header.push(format!("{:>7}", "mem"));
    header.push("  state".to_owned());
    let _ = writeln!(text, "{}", header.concat());

    for (total, weakest, name, measured) in order {
        let numbers = &measured.numbers;
        let reading = score::reading(numbers);
        let score = if measured.gates.ok() {
            format!("{total:.3}")
        } else {
            "gate".to_owned()
        };
        let weak = if measured.gates.ok() {
            format!("{weakest:.2}")
        } else {
            "—".to_owned()
        };

        let state = if !measured.gates.ok() {
            "failed"
        } else if timing && !weights.floors.speed(numbers) {
            "slow"
        } else if !weights.floors.memory(numbers) {
            "heavy"
        } else {
            "ok"
        };

        let speed = numbers.speed();
        let speed_text = if speed >= TOO_FAST {
            "fast".to_owned()
        } else {
            format!("{speed:.1}")
        };

        // A column that is not in the table is not printed as an empty one: the width would be a
        // hole in a committed record, where every difference between two files is one to notice.
        let mut cells = vec![
            format!("  {name:<NAME$}"),
            format!("{score:>7}"),
            format!("{weak:>7}"),
            format!("{:>7.3}", reading.dedup),
            format!("{:>9.3}", reading.worst_marginal),
            format!("{:>8.3}", reading.stored),
            format!("{:>9.3}", reading.manifest),
            format!("{:>8}", numbers.chunks),
        ];
        if timing {
            cells.push(format!("{speed_text:>9}"));
        }
        cells.push(format!("{:>7.2}", numbers.worst_peak));
        cells.push(format!("  {state}"));

        let _ = writeln!(text, "{}", cells.concat());
    }

    for (name, measured) in rows {
        if measured.gates.ok() {
            continue;
        }

        let _ = writeln!(
            text,
            "  {name}: covers={} stable={} round-trip={}",
            measured.gates.covers, measured.gates.stable, measured.gates.round_trip,
        );
    }

    text
}

/// Measures every chunker through a real store and compares it with the model's prediction.
///
/// # Errors
///
/// Returns whatever writing the chain into a store, or reading it back, fails with.
async fn check_against_store(
    pattern: &Pattern,
    chunkings: &[(&'static str, rorolala_storage::Chunking)],
    rows: &[(&'static str, Measured)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut agreed = 0;
    let mut differed = 0;

    for ((name, chunking), (_, measured)) in chunkings.iter().zip(rows) {
        let written = store::measure(pattern, *chunking).await?;

        if written.stored == measured.numbers.stored {
            agreed += 1;
        } else {
            differed += 1;
            println!(
                "  store {name}: model {} but the store holds {}",
                measured.numbers.stored, written.stored
            );
        }
    }

    println!(
        "  store: {agreed} of {} chunkers agreed with the model",
        agreed + differed
    );

    Ok(())
}

/// The bytes every version of the pattern is.
///
/// # Errors
///
/// Returns whatever reading a version's size fails with.
fn total_bytes(pattern: &Pattern) -> Result<u64, std::io::Error> {
    let mut total = 0_u64;

    for path in pattern.versions() {
        total = total.saturating_add(fs::metadata(path)?.len());
    }

    Ok(total)
}

/// A short digest of everything the corpus is made of: the names and the bytes, in the order read.
///
/// It is what a score is taken against, so it is printed with one: a score whose corpus moved is a
/// score of something else, and the digest is how a reader sees that from the report alone.
fn corpus_hash(policy: &Policy) -> Result<String, std::io::Error> {
    let mut hasher = blake3::Hasher::new();

    for pattern in &policy.patterns {
        hasher.update(pattern.name.as_bytes());

        for lane in &pattern.lanes {
            hasher.update(lane.name.as_bytes());

            for path in &lane.versions {
                if let Some(name) = path.file_name() {
                    hasher.update(name.to_string_lossy().as_bytes());
                }
                hasher.update(&fs::read(path)?);
            }
        }
    }

    let digest = hasher.finalize().to_hex().to_string();

    Ok(digest.get(..16).unwrap_or(&digest).to_owned())
}

#[cfg(test)]
mod tests {
    use super::corpus_hash;
    use crate::corpus::Corpus;
    use crate::generate::{Scale, synthetic};

    #[test]
    fn a_corpus_hash_moves_when_a_version_does() {
        let root = crate::scratch::at("hash");

        synthetic(&root, "synthetic", Scale::Test).unwrap();
        let corpus = Corpus::read(&root).unwrap();
        let before = corpus_hash(&corpus.policies[0]).unwrap();

        // The same corpus read twice is the same digest, which is what makes it usable as "these are
        // the bytes the score was taken against".
        assert_eq!(before, corpus_hash(&corpus.policies[0]).unwrap());
        assert_eq!(before.len(), 16);

        let version = root.join("synthetic/big-file/000.bin");
        let mut bytes = std::fs::read(&version).unwrap();
        let last = bytes.last_mut().unwrap();
        *last ^= 0xff;
        std::fs::write(&version, bytes).unwrap();

        let corpus = Corpus::read(&root).unwrap();
        assert_ne!(before, corpus_hash(&corpus.policies[0]).unwrap());
    }
}
