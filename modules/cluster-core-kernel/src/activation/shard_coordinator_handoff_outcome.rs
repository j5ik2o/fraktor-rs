//! Result of one shard coordinator handoff round.

use alloc::string::String;

/// Result of one handoff round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardCoordinatorHandoffOutcome {
  /// Shard identifier that completed handoff.
  pub shard_id: String,
  /// Whether handoff completed successfully.
  pub success:  bool,
}
