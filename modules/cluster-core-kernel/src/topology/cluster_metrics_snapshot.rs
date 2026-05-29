//! Immutable snapshot of collected cluster metrics.

/// Read-only metrics view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClusterMetricsSnapshot {
  members:        usize,
  virtual_actors: i64,
}

impl ClusterMetricsSnapshot {
  /// Internal constructor used by metrics collector.
  pub(crate) const fn new(members: usize, virtual_actors: i64) -> Self {
    Self { members, virtual_actors }
  }

  /// Member count.
  #[must_use]
  pub const fn members(&self) -> usize {
    self.members
  }

  /// Virtual actor count.
  #[must_use]
  pub const fn virtual_actors(&self) -> i64 {
    self.virtual_actors
  }
}
