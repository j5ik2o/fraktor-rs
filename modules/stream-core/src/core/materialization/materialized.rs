use super::{SharedKillSwitch, UniqueKillSwitch};
use crate::core::r#impl::materialization::StreamHandleImpl;

/// Result of materializing a stream graph.
pub struct Materialized<Mat> {
  handle:       StreamHandleImpl,
  materialized: Mat,
}

impl<Mat> Materialized<Mat> {
  pub(crate) const fn new(handle: StreamHandleImpl, materialized: Mat) -> Self {
    Self { handle, materialized }
  }

  /// Returns the stream handle.
  #[must_use]
  pub const fn handle(&self) -> &StreamHandleImpl {
    &self.handle
  }

  /// Returns the materialized value.
  #[must_use]
  pub const fn materialized(&self) -> &Mat {
    &self.materialized
  }

  /// Consumes this handle and returns the owned materialized value.
  #[must_use]
  pub fn into_materialized(self) -> Mat {
    self.materialized
  }

  /// Returns a unique kill switch bound to this materialized stream.
  #[must_use]
  pub fn unique_kill_switch(&self) -> UniqueKillSwitch {
    self.handle.unique_kill_switch()
  }

  /// Returns a shared kill switch bound to this materialized stream.
  #[must_use]
  pub fn shared_kill_switch(&self) -> SharedKillSwitch {
    self.handle.shared_kill_switch()
  }
}
