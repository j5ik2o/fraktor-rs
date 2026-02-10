use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{SharedKillSwitch, StreamHandleGeneric, UniqueKillSwitch};

/// Result of materializing a stream graph.
pub struct Materialized<Mat, TB: RuntimeToolbox> {
  handle:       StreamHandleGeneric<TB>,
  materialized: Mat,
}

impl<Mat, TB: RuntimeToolbox> Materialized<Mat, TB> {
  pub(crate) const fn new(handle: StreamHandleGeneric<TB>, materialized: Mat) -> Self {
    Self { handle, materialized }
  }

  /// Returns the stream handle.
  #[must_use]
  pub const fn handle(&self) -> &StreamHandleGeneric<TB> {
    &self.handle
  }

  /// Returns the materialized value.
  #[must_use]
  pub const fn materialized(&self) -> &Mat {
    &self.materialized
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
