//! Outbound action emitted by the shard coordinator handoff state machine.

use alloc::{collections::BTreeSet, string::String};

/// Outbound action emitted by the handoff state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardCoordinatorHandoffAction {
  /// Broadcast begin-hand-off to all regions.
  SendBeginHandOff {
    /// Shard identifier being moved.
    shard_id: String,
    /// Regions that must receive begin-hand-off.
    regions:  BTreeSet<String>,
  },
  /// Instruct the source region to stop hosting the shard.
  SendHandOff {
    /// Shard identifier being moved.
    shard_id:      String,
    /// Region that must stop the shard.
    source_region: String,
  },
}
