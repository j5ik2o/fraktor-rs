//! Cluster metrics state (members/virtual actors) with optional collection.

use crate::core::cluster_metrics_snapshot::ClusterMetricsSnapshot;

/// Metrics state when enabled.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClusterMetrics {
  members:        usize,
  virtual_actors: i64,
}

impl ClusterMetrics {
  /// Creates empty metrics.
  #[must_use]
  pub const fn new() -> Self {
    Self { members: 0, virtual_actors: 0 }
  }

  /// Updates member count.
  pub const fn update_members(&mut self, count: usize) {
    self.members = count;
  }

  /// Updates virtual actor count.
  pub const fn update_virtual_actors(&mut self, count: i64) {
    self.virtual_actors = count;
  }

  /// Returns a snapshot of the current metrics.
  #[must_use]
  pub const fn snapshot(&self) -> ClusterMetricsSnapshot {
    ClusterMetricsSnapshot::new(self.members, self.virtual_actors)
  }
}
