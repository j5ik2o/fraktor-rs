//! Versioned membership delta.

use alloc::vec::Vec;

use crate::core::{membership_version::MembershipVersion, node_record::NodeRecord};

/// Represents a set of membership changes between two versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipDelta {
  /// Source version before applying the delta.
  pub from:    MembershipVersion,
  /// Target version after applying the delta.
  pub to:      MembershipVersion,
  /// Records updated by this delta.
  pub entries: Vec<NodeRecord>,
}

impl MembershipDelta {
  /// Creates a delta.
  #[must_use]
  pub const fn new(from: MembershipVersion, to: MembershipVersion, entries: Vec<NodeRecord>) -> Self {
    Self { from, to, entries }
  }
}
