//! Events emitted by membership changes.

use alloc::string::String;

/// Event kinds used to feed EventStream/metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembershipEvent {
  /// Authority collision detected during join.
  AuthorityConflict {
    /// Authority that conflicted.
    authority:         String,
    /// Node id already holding the authority.
    existing_node_id:  String,
    /// Node id that attempted to join with the same authority.
    requested_node_id: String,
  },
  /// Node joined and became `Up`.
  Joined {
    /// Joined node id.
    node_id:   String,
    /// Authority assigned to the node.
    authority: String,
  },
  /// Node left gracefully and was removed.
  Left {
    /// Leaving node id.
    node_id:   String,
    /// Authority associated with the leaving node.
    authority: String,
  },
  /// Node marked unreachable after heartbeat misses.
  MarkedUnreachable {
    /// Node id considered unreachable.
    node_id:   String,
    /// Authority of the unreachable node.
    authority: String,
  },
}
