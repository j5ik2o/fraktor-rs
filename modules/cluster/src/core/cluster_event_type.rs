//! Cluster event categories used by subscription filters.

use crate::core::ClusterEvent;

/// Supported cluster event categories for `ClusterApi::subscribe`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ClusterEventType {
  /// Matches [`ClusterEvent::Startup`].
  Startup,
  /// Matches [`ClusterEvent::StartupFailed`].
  StartupFailed,
  /// Matches [`ClusterEvent::Shutdown`].
  Shutdown,
  /// Matches [`ClusterEvent::ShutdownFailed`].
  ShutdownFailed,
  /// Matches [`ClusterEvent::TopologyUpdated`].
  TopologyUpdated,
  /// Matches [`ClusterEvent::MemberStatusChanged`].
  MemberStatusChanged,
  /// Matches [`ClusterEvent::CurrentClusterState`].
  CurrentClusterState,
  /// Matches [`ClusterEvent::SeenChanged`].
  SeenChanged,
  /// Matches [`ClusterEvent::UnreachableMember`].
  UnreachableMember,
  /// Matches [`ClusterEvent::ReachableMember`].
  ReachableMember,
  /// Matches [`ClusterEvent::MemberQuarantined`].
  MemberQuarantined,
  /// Matches [`ClusterEvent::TopologyApplyFailed`].
  TopologyApplyFailed,
}

impl ClusterEventType {
  /// Returns `true` when this filter category matches the provided cluster event.
  #[must_use]
  pub const fn matches(self, event: &ClusterEvent) -> bool {
    match (self, event) {
      | (Self::Startup, ClusterEvent::Startup { .. })
      | (Self::StartupFailed, ClusterEvent::StartupFailed { .. })
      | (Self::Shutdown, ClusterEvent::Shutdown { .. })
      | (Self::ShutdownFailed, ClusterEvent::ShutdownFailed { .. })
      | (Self::TopologyUpdated, ClusterEvent::TopologyUpdated { .. })
      | (Self::MemberStatusChanged, ClusterEvent::MemberStatusChanged { .. })
      | (Self::CurrentClusterState, ClusterEvent::CurrentClusterState { .. })
      | (Self::SeenChanged, ClusterEvent::SeenChanged { .. })
      | (Self::UnreachableMember, ClusterEvent::UnreachableMember { .. })
      | (Self::ReachableMember, ClusterEvent::ReachableMember { .. })
      | (Self::MemberQuarantined, ClusterEvent::MemberQuarantined { .. })
      | (Self::TopologyApplyFailed, ClusterEvent::TopologyApplyFailed { .. }) => true,
      // NOTE: ワイルドカードは意図的。ClusterEvent/ClusterEventType にバリアントを追加する場合は
      // 対応する行もここに追加すること。
      | _ => false,
    }
  }
}
