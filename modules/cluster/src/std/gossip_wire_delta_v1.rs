//! Wire representation of membership delta.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::{
  core::membership::{MembershipDelta, MembershipVersion},
  std::gossip_wire_node_record::GossipWireNodeRecord,
};

/// Wire representation of a membership delta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GossipWireDeltaV1 {
  /// Source version before applying the delta.
  pub from:    u64,
  /// Target version after applying the delta.
  pub to:      u64,
  /// Records updated by this delta.
  pub entries: Vec<GossipWireNodeRecord>,
}

impl GossipWireDeltaV1 {
  pub(crate) fn from_delta(delta: &MembershipDelta) -> Self {
    let entries = delta.entries.iter().map(GossipWireNodeRecord::from_record).collect();
    Self { from: delta.from.value(), to: delta.to.value(), entries }
  }

  pub(crate) fn to_delta(&self) -> Option<MembershipDelta> {
    let mut entries = Vec::with_capacity(self.entries.len());
    for entry in &self.entries {
      entries.push(entry.to_record()?);
    }
    Some(MembershipDelta::new(MembershipVersion::new(self.from), MembershipVersion::new(self.to), entries))
  }
}
