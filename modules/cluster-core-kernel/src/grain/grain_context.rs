//! Grain execution context abstraction.

use crate::ClusterApi;

/// Execution context exposed to grain implementations.
pub trait GrainContext {
  /// Returns the grain kind.
  fn kind(&self) -> &str;
  /// Returns the grain identity.
  fn identity(&self) -> &str;
  /// Returns the cluster API reference.
  fn cluster(&self) -> &ClusterApi;
}
