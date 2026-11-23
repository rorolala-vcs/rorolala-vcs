//! The chunking policies this build carries, as the C ABI reads them.
//!
//! A policy is a named way of cutting content, and which containers each claims is what a caller
//! needs to know to choose one or to explain a cut. The table is `rorolala_chunking`'s, generated
//! from the policy files themselves; the reading lives here rather than there because that crate is
//! deliberately free of dependencies.

use rorolala_chunking::{POLICIES, PolicyEntry};
use rorolala_utils_lazyffi::lazyffi;

/// Every chunking policy this build carries.
///
/// Opaque: C takes a pointer from `rola_chunking_policies_new` and gives it back with
/// `free_rola_chunking_policies`. A policy is read by index, and the containers it claims by a
/// second index within it, so neither the policy nor its list of containers is ever copied whole.
#[lazyffi(export = RolaChunkingPolicies)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ChunkingPolicies;

/// Makes the list of this build's chunking policies.
#[must_use]
#[lazyffi(export = rola_chunking_policies_new)]
pub const fn policies_new() -> ChunkingPolicies {
    ChunkingPolicies
}

#[lazyffi(export = rola_)]
impl ChunkingPolicies {
    /// How many policies this build carries.
    #[must_use]
    pub fn count(&self) -> usize {
        POLICIES.len()
    }

    /// The name of the policy at `index`, or an empty string past the end.
    #[must_use]
    pub fn name_at(&self, index: usize) -> String {
        policy(index).map_or_else(String::new, |policy| policy.name.to_owned())
    }

    /// How many containers the policy at `index` claims, or zero past the end.
    #[must_use]
    pub fn extension_count(&self, index: usize) -> usize {
        policy(index).map_or(0, |policy| policy.extensions.len())
    }

    /// The container the policy at `index` claims at `extension`, or an empty string past either end.
    #[must_use]
    pub fn extension_at(&self, index: usize, extension: usize) -> String {
        policy(index)
            .and_then(|policy| policy.extensions.get(extension))
            .map_or_else(String::new, |name| (*name).to_owned())
    }
}

/// The policy at `index`, when the table holds one.
fn policy(index: usize) -> Option<&'static PolicyEntry> {
    POLICIES.get(index)
}

#[cfg(test)]
mod tests {
    use rorolala_chunking::POLICIES;

    use super::policies_new;

    #[test]
    fn every_policy_is_listed_with_what_it_claims() {
        let policies = policies_new();

        assert_eq!(policies.count(), POLICIES.len());

        for (index, policy) in POLICIES.iter().enumerate() {
            assert_eq!(policies.name_at(index), policy.name);
            assert_eq!(policies.extension_count(index), policy.extensions.len());

            for (at, extension) in policy.extensions.iter().enumerate() {
                assert_eq!(policies.extension_at(index, at), *extension);
            }
        }
    }

    #[test]
    fn reading_past_the_end_answers_with_nothing() {
        let policies = policies_new();

        assert_eq!(policies.name_at(POLICIES.len()), "");
        assert_eq!(policies.extension_count(POLICIES.len()), 0);
        assert_eq!(policies.extension_at(POLICIES.len(), 0), "");
        assert_eq!(policies.extension_at(0, usize::MAX), "");
    }
}
