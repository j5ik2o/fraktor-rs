use super::{SharedKillSwitch, UniqueKillSwitch};
use crate::core::r#impl::materialization::StreamShared;

/// Result of materializing a stream graph.
pub struct Materialized<Mat> {
  stream:       StreamShared,
  materialized: Mat,
}

impl<Mat> Materialized<Mat> {
  pub(crate) const fn new(stream: StreamShared, materialized: Mat) -> Self {
    Self { stream, materialized }
  }

  /// Returns the stream bound to this materialized value.
  #[must_use]
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

  /// Returns a unique kill switch bound to this materialized stream.
  #[must_use]
  pub fn unique_kill_switch(&self) -> UniqueKillSwitch {
    self.stream().unique_kill_switch()
  }

  /// Returns a shared kill switch bound to this materialized stream.
  #[must_use]
  pub fn shared_kill_switch(&self) -> SharedKillSwitch {
    self.stream().shared_kill_switch()
  }
}
