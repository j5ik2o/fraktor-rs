//! Member ordering: deterministic total order by membership age.

#[cfg(test)]
#[path = "member_ordering_test.rs"]
mod tests;

use alloc::vec::Vec;
use core::cmp::Ordering;

use super::NodeRecord;

/// Total order comparison by membership age (oldest first).
///
/// Delegates to [`NodeRecord::is_older_than`] to derive an [`Ordering`]
/// that satisfies antisymmetry, transitivity, and totality.
#[must_use]
pub fn member_age_order(a: &NodeRecord, b: &NodeRecord) -> Ordering {
  if a.is_older_than(b) {
    Ordering::Less
  } else if b.is_older_than(a) {
    Ordering::Greater
  } else {
    Ordering::Equal
  }
}

/// Returns records sorted oldest-first. Input order does not affect the result.
#[must_use]
pub fn age_ordered<'a>(records: &'a [NodeRecord]) -> Vec<&'a NodeRecord> {
  let mut refs: Vec<&'a NodeRecord> = records.iter().collect();
  refs.sort_by(|a, b| member_age_order(a, b));
  refs
}

/// Returns the oldest member, or `None` when `records` is empty.
#[must_use]
pub fn oldest_member(records: &[NodeRecord]) -> Option<&NodeRecord> {
  age_ordered(records).into_iter().next()
}
