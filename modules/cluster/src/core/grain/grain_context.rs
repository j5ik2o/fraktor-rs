//! Grain execution context abstraction.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::ClusterApiGeneric;

/// Execution context exposed to grain implementations.
pub trait GrainContext<TB: RuntimeToolbox + 'static> {
  /// Returns the grain kind.
  fn kind(&self) -> &str;
  /// Returns the grain identity.
  fn identity(&self) -> &str;
  /// Returns the cluster API reference.
  fn cluster(&self) -> &ClusterApiGeneric<TB>;
}
