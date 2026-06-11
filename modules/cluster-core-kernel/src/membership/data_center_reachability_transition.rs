//! Data center reachability transition output.

#[cfg(test)]
#[path = "data_center_reachability_transition_test.rs"]
mod tests;

use fraktor_utils_core_rs::time::TimerInstant;

use super::DataCenter;
use crate::topology::ClusterEvent;

/// Latched transition output for data center level reachability changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataCenterReachabilityTransition {
  /// The data center became unreachable (all observed targets unreachable).
  BecameUnreachable {
    /// The data center whose reachability changed.
    data_center: DataCenter,
  },
  /// The data center became reachable again (at least one target reachable).
  BecameReachable {
    /// The data center whose reachability changed.
    data_center: DataCenter,
  },
}

impl DataCenterReachabilityTransition {
  /// Converts this transition into the corresponding [`ClusterEvent`].
  ///
  /// The `observed_at` timestamp is attached to the resulting event.
  #[must_use]
  pub fn to_cluster_event(self, observed_at: TimerInstant) -> ClusterEvent {
    match self {
      | Self::BecameUnreachable { data_center } => ClusterEvent::UnreachableDataCenter { data_center, observed_at },
      | Self::BecameReachable { data_center } => ClusterEvent::ReachableDataCenter { data_center, observed_at },
    }
  }
}
