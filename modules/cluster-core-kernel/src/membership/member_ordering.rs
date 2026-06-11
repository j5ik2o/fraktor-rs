//! Member ordering: deterministic total order by membership age.

#[cfg(test)]
#[path = "member_ordering_test.rs"]
mod tests;

use alloc::vec::Vec;
use core::cmp::Ordering;

use super::NodeRecord;

/// Total order comparison by membership age (oldest first).
///
/// Delegates to [`NodeRecord::is_older_than`] for the age semantics
/// (join version, then authority tie-break), with a final tie-break on
/// the unique address so that distinct incarnations of the same authority
/// still form a deterministic total order.
#[must_use]
pub fn member_age_order(a: &NodeRecord, b: &NodeRecord) -> Ordering {
  if a.is_older_than(b) {
    Ordering::Less
  } else if b.is_older_than(a) {
    Ordering::Greater
  } else {
    // is_older_than が相互に false（join_version と authority が同一）でも、
    // gossip merge では別 incarnation の record が併存し得るため、
    // unique_address（incarnation 込み）で最終 tie-break して全順序を保つ
    a.unique_address.cmp(&b.unique_address)
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
  records.iter().min_by(|a, b| member_age_order(a, b))
}
