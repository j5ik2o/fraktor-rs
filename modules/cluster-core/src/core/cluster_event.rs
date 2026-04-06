//! Cluster lifecycle and topology events emitted to the event stream.

use alloc::{string::String, vec::Vec};

use fraktor_utils_rs::core::time::TimerInstant;

use crate::core::{
  TopologyUpdate,
  membership::{CurrentClusterState, MembershipVersion, NodeStatus},
  startup_mode::StartupMode,
};

/// Event payload published via `EventStreamEvent::Extension { name: "cluster", .. }`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClusterEvent {
  /// Cluster startup succeeded.
  Startup {
    /// Advertised address.
    address: String,
    /// Startup mode.
    mode:    StartupMode,
  },
  /// Cluster startup failed.
  StartupFailed {
    /// Advertised address.
    address: String,
    /// Startup mode.
    mode:    StartupMode,
    /// Failure reason.
    reason:  String,
  },
  /// Cluster shutdown succeeded.
  Shutdown {
    /// Advertised address.
    address: String,
    /// Shutdown mode.
    mode:    StartupMode,
  },
  /// Cluster shutdown failed.
  ShutdownFailed {
    /// Advertised address.
    address: String,
    /// Shutdown mode.
    mode:    StartupMode,
    /// Failure reason.
    reason:  String,
  },
  /// Topology changed.
  TopologyUpdated {
    /// Topology update payload.
    update: TopologyUpdate,
  },
  /// Member status changed.
  MemberStatusChanged {
    /// Node identifier.
    node_id:     String,
    /// Authority address.
    authority:   String,
    /// Previous status.
    from:        NodeStatus,
    /// Current status.
    to:          NodeStatus,
    /// Observation timestamp.
    observed_at: TimerInstant,
  },
  /// Current cluster state snapshot.
  CurrentClusterState {
    /// Enriched current cluster state.
    state:       CurrentClusterState,
    /// Observation timestamp.
    observed_at: TimerInstant,
  },
  /// Gossip seen-set changed for a version.
  SeenChanged {
    /// Authorities that have seen the version.
    seen_by:     Vec<String>,
    /// Membership version associated with the seen-set.
    version:     MembershipVersion,
    /// Observation timestamp.
    observed_at: TimerInstant,
  },
  /// Member became unreachable.
  UnreachableMember {
    /// Node identifier.
    node_id:     String,
    /// Authority address.
    authority:   String,
    /// Observation timestamp.
    observed_at: TimerInstant,
  },
  /// Member became reachable again.
  ReachableMember {
    /// Node identifier.
    node_id:     String,
    /// Authority address.
    authority:   String,
    /// Observation timestamp.
    observed_at: TimerInstant,
  },
  /// Member quarantined.
  MemberQuarantined {
    /// Authority address.
    authority:   String,
    /// Quarantine reason.
    reason:      String,
    /// Observation timestamp.
    observed_at: TimerInstant,
  },
  /// Topology apply failed.
  TopologyApplyFailed {
    /// Failure reason.
    reason:      String,
    /// Observation timestamp.
    observed_at: TimerInstant,
  },
}
