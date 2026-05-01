use fraktor_utils_core_rs::core::sync::SharedAccess;

use super::{SharedKillSwitch, UniqueKillSwitch};
use crate::core::r#impl::materialization::StreamShared;

#[cfg(test)]
mod tests;

/// Result of materializing a stream graph.
pub struct Materialized<Mat> {
  stream:       StreamShared,
  materialized: Mat,
}

impl<Mat> Materialized<Mat> {
  pub(in crate::core) const fn new(stream: StreamShared, materialized: Mat) -> Self {
    Self { stream, materialized }
  }

  /// Returns the materialized value.
  #[must_use]
  pub const fn materialized(&self) -> &Mat {
    &self.materialized
  }

  /// Consumes this value and returns the owned materialized value.
  #[must_use]
  pub fn into_materialized(self) -> Mat {
    self.materialized
  }

  /// Returns a unique kill switch bound to this materialized stream graph.
  ///
  /// For a graph split into multiple islands, this switch represents the
  /// whole materialized graph. Shutdown and abort signals are propagated to
  /// every island actor, and terminal state is derived for the graph as a
  /// whole.
  #[must_use]
  pub fn unique_kill_switch(&self) -> UniqueKillSwitch {
    let state = self.stream.with_read(|stream| stream.kill_switch_state());
    UniqueKillSwitch::from_state(state)
  }

  /// Returns a shared kill switch bound to this materialized stream graph.
  ///
  /// For a graph split into multiple islands, this switch represents the
  /// whole materialized graph. Shutdown and abort signals are propagated to
  /// every island actor, and terminal state is derived for the graph as a
  /// whole.
  #[must_use]
  pub fn shared_kill_switch(&self) -> SharedKillSwitch {
    let state = self.stream.with_read(|stream| stream.kill_switch_state());
    SharedKillSwitch::from_state(state)
  }
}
