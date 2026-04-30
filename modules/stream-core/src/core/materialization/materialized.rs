#[cfg(any(test, feature = "test-support"))]
use fraktor_utils_core_rs::core::sync::SharedAccess;

use super::{SharedKillSwitch, UniqueKillSwitch};
use crate::core::{KillSwitchStateHandle, r#impl::materialization::StreamShared};

/// Result of materializing a stream graph.
pub struct Materialized<Mat> {
  #[cfg(any(test, feature = "test-support"))]
  stream:            StreamShared,
  kill_switch_state: KillSwitchStateHandle,
  materialized:      Mat,
}

impl<Mat> Materialized<Mat> {
  #[cfg(any(test, feature = "test-support"))]
  pub(crate) fn new(stream: StreamShared, materialized: Mat) -> Self {
    let kill_switch_state = stream.with_read(|stream| stream.kill_switch_state());
    Self { stream, kill_switch_state, materialized }
  }

  #[cfg(any(test, feature = "test-support"))]
  pub(in crate::core) const fn new_with_kill_switch_state(
    stream: StreamShared,
    kill_switch_state: KillSwitchStateHandle,
    materialized: Mat,
  ) -> Self {
    Self { stream, kill_switch_state, materialized }
  }

  #[cfg(not(any(test, feature = "test-support")))]
  pub(in crate::core) fn new_with_kill_switch_state(
    _stream: StreamShared,
    kill_switch_state: KillSwitchStateHandle,
    materialized: Mat,
  ) -> Self {
    Self { kill_switch_state, materialized }
  }

  /// Returns the stream bound to this materialized value.
  #[must_use]
  #[cfg(any(test, feature = "test-support"))]
  pub(crate) const fn stream(&self) -> &StreamShared {
    &self.stream
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
    UniqueKillSwitch::from_state(self.kill_switch_state.clone())
  }

  /// Returns a shared kill switch bound to this materialized stream graph.
  ///
  /// For a graph split into multiple islands, this switch represents the
  /// whole materialized graph. Shutdown and abort signals are propagated to
  /// every island actor, and terminal state is derived for the graph as a
  /// whole.
  #[must_use]
  pub fn shared_kill_switch(&self) -> SharedKillSwitch {
    SharedKillSwitch::from_state(self.kill_switch_state.clone())
  }
}
