//! Deploy scope for cluster-aware routing.

#[cfg(test)]
#[path = "cluster_scope_test.rs"]
mod tests;

/// Deploy scope indicating that an actor should be routed through the cluster layer.
///
/// This corresponds to Pekko's `ClusterScope` and is used by cluster router deploy
/// configurations to mark actors as cluster-aware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClusterScope;

impl ClusterScope {
  /// Returns the singleton cluster scope instance.
  #[must_use]
  pub const fn instance() -> Self {
    Self
  }
}
