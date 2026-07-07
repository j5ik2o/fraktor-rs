//! Command observed by the shard coordinator handoff state machine.

use alloc::{collections::BTreeSet, string::String};

/// Command observed by the shard coordinator handoff state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardCoordinatorHandoffCommand {
  /// Starts handoff for one shard.
  Start {
    /// Shard identifier being moved.
    shard_id:      String,
    /// Region currently hosting the shard.
    source_region: String,
    /// Regions that must acknowledge begin-hand-off.
    regions:       BTreeSet<String>,
  },
  /// Acknowledgement of begin-hand-off from one region.
  BeginHandOffAck {
    /// Shard identifier being moved.
    shard_id: String,
    /// Region that acknowledged begin-hand-off.
    region:   String,
  },
  /// Notification that all entities in the shard stopped.
  ShardStopped {
    /// Shard identifier that stopped.
    shard_id: String,
  },
  /// Notification that a region terminated during handoff.
  RegionTerminated {
    /// Region that terminated.
    region: String,
  },
  /// Handoff timed out.
  Timeout,
}
