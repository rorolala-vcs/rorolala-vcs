//! The chunking policies this build carries, as the C ABI reads them.
//!
//! A policy is a named way of cutting content, and which containers each claims is what a caller
//! needs to know to choose one or to explain a cut. The table is `rorolala_chunking`'s, generated
//! from the policy files themselves; the reading lives here rather than there because that crate is
//! deliberately free of dependencies, and writing JSON would drag `serde` into it.

use rorolala_chunking::POLICIES;
use rorolala_utils_lazyffi::lazyffi;
use serde::Serialize;

/// One policy, as the C ABI writes it.
#[derive(Debug, Serialize)]
struct PolicyItem {
    /// The name a store is told to use it by.
    name: &'static str,
    /// The containers it claims.
    extensions: &'static [&'static str],
}

/// Every chunking policy this build carries, in name order.
///
/// # FFI
///
/// An owned JSON array the caller releases with `free_string`,
/// `[{"name":"psd-basic","extensions":["psd","psb"]}]`. The order is the names', which is the order
/// a report, a completion list and a refusal name the policies in.
#[must_use]
#[lazyffi(export = rola_chunking_policies_json)]
pub fn policies_json() -> String {
    let policies: Vec<PolicyItem> = POLICIES
        .iter()
        .map(|policy| PolicyItem {
            name: policy.name,
            extensions: policy.extensions,
        })
        .collect();

    // Writing this view cannot fail: it is strings and lists of strings. A failure would answer
    // `null`, which a caller reads as no policies rather than as a document.
    serde_json::to_string(&policies).unwrap_or_else(|_| "null".to_owned())
}

#[cfg(test)]
mod tests {
    use rorolala_chunking::POLICIES;

    use super::policies_json;

    #[test]
    fn every_policy_is_listed_with_what_it_claims() {
        let answer: serde_json::Value = serde_json::from_str(&policies_json()).unwrap();
        let policies = answer.as_array().unwrap();

        assert_eq!(policies.len(), POLICIES.len());

        for (listed, policy) in policies.iter().zip(POLICIES) {
            assert_eq!(listed["name"], policy.name);
            assert_eq!(
                listed["extensions"].as_array().unwrap().len(),
                policy.extensions.len()
            );
        }
    }
}
