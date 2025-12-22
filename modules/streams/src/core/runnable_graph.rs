//! Runnable graph representation.

use alloc::vec::Vec;

use crate::core::mat_combine::MatCombine;

/// Graph that can be materialized for execution.
#[derive(Debug)]
pub struct RunnableGraph {
  connections: Vec<(u64, u64, MatCombine)>,
  mat_value:   MatCombine,
}

impl RunnableGraph {
  pub(crate) const fn new(connections: Vec<(u64, u64, MatCombine)>, mat_value: MatCombine) -> Self {
    Self { connections, mat_value }
  }

  /// Returns the materialized value derived from combine rules.
  #[must_use]
  pub const fn materialized_value(&self) -> MatCombine {
    self.mat_value
  }

  /// Returns the number of connections.
  #[must_use]
  pub const fn connection_count(&self) -> usize {
    self.connections.len()
  }
}
