//! Membership snapshot for handshake.

use alloc::vec::Vec;

use crate::core::{membership_version::MembershipVersion, node_record::NodeRecord};

/// Immutable view of the membership table used during handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipSnapshot {
  /// Version when the snapshot was taken.
  pub version: MembershipVersion,
  /// Complete list of node records.
  pub entries: Vec<NodeRecord>,
}

impl MembershipSnapshot {
  /// Creates a new snapshot.
  pub fn new(version: MembershipVersion, entries: Vec<NodeRecord>) -> Self {
    Self { version, entries }
  }
}
