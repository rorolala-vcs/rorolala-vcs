//! The order two rule files are applied in when they sit beside each other.
//!
//! A name with numbers in it is ordered the way a reader expects — `2` before `10` — which a
//! plain lexicographic ordering does not do. The comparison is byte-oriented rather than
//! locale-aware: rule files are named by whoever wrote them, and two runs of the loader have to
//! agree about one name set, not about any language's idea of order.

use std::cmp::Ordering;

/// Compares `left` and `right` naturally: runs of digits by value, everything else byte by byte.
///
/// Two digit runs equal in value are told apart by length, so the ordering stays total — `01`
/// and `1` are two names, and one of them has to come first. Shorter goes first there, which is
/// what the plain byte ordering would have said as well.
pub fn natural(left: &str, right: &str) -> Ordering {
    let mut left = left.as_bytes();
    let mut right = right.as_bytes();

    loop {
        match (left.first(), right.first()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&a), Some(&b)) => {
                if a.is_ascii_digit() && b.is_ascii_digit() {
                    let (la, rest_a) = digits(left);
                    let (lb, rest_b) = digits(right);
                    let (va, vb) = (without_zeroes(la), without_zeroes(lb));

                    // A run of digits long enough to overflow is compared by text: once the leading
                    // zeroes are off, the longer run is the larger number, and equal-length runs are
                    // settled byte by byte, which for digits is by value. Runs equal in value are
                    // told apart by how they were written.
                    let by_value = va
                        .len()
                        .cmp(&vb.len())
                        .then_with(|| va.cmp(vb))
                        .then_with(|| la.len().cmp(&lb.len()));

                    if by_value != Ordering::Equal {
                        return by_value;
                    }

                    left = rest_a;
                    right = rest_b;
                } else {
                    match a.cmp(&b) {
                        Ordering::Equal => {
                            left = &left[1..];
                            right = &right[1..];
                        }
                        other => return other,
                    }
                }
            }
        }
    }
}

/// The leading run of digits of `bytes`, and what follows it.
fn digits(bytes: &[u8]) -> (&[u8], &[u8]) {
    let end = bytes
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(bytes.len());

    bytes.split_at(end)
}

/// `run` without its leading zeroes: the number it is, as the shortest text that writes it.
fn without_zeroes(run: &[u8]) -> &[u8] {
    let start = run
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(run.len());

    &run[start..]
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::natural;

    #[test]
    fn numbers_order_by_value() {
        assert_eq!(natural("2", "10"), Ordering::Less);
        assert_eq!(natural("10", "2"), Ordering::Greater);
        assert_eq!(natural("a2", "a10"), Ordering::Less);
    }

    #[test]
    fn letters_order_by_byte() {
        assert_eq!(natural("abc", "abd"), Ordering::Less);
        assert_eq!(natural("b", "a"), Ordering::Greater);
    }

    #[test]
    fn a_prefix_comes_first() {
        assert_eq!(natural("a", "ab"), Ordering::Less);
        assert_eq!(natural("ab", "a"), Ordering::Greater);
    }

    #[test]
    fn equal_names_order_equal() {
        assert_eq!(natural("maya", "maya"), Ordering::Equal);
        assert_eq!(natural("a10", "a10"), Ordering::Equal);
    }

    #[test]
    fn leading_zeroes_do_not_change_the_value_but_stay_ordered() {
        // Equal in value, so how they are written decides: the shorter spelling comes first.
        assert_eq!(natural("01", "1"), Ordering::Greater);
        assert_eq!(natural("1", "01"), Ordering::Less);
        assert_eq!(natural("010", "10"), Ordering::Greater);
    }

    #[test]
    fn a_digit_run_longer_than_a_number_still_orders() {
        let long = "9".repeat(40);
        let longer = "1".repeat(41);

        assert_eq!(natural(&long, &longer), Ordering::Less);
    }

    #[test]
    fn mixed_runs_compare_run_by_run() {
        assert_eq!(natural("a2b10", "a2b9"), Ordering::Greater);
        assert_eq!(natural("a2b", "a10a"), Ordering::Less);
    }
}
