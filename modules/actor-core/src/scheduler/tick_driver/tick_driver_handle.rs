//! Handle owning the lifetime of a running tick driver instance.

use core::time::Duration;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{TickDriverControl, TickDriverId, TickDriverKind};

/// Handle owning the lifetime of a running tick driver instance.
#[derive(Clone)]
pub struct TickDriverHandle {
  id:         TickDriverId,
  kind:       TickDriverKind,
  resolution: Duration,
  control:    ArcShared<dyn TickDriverControl>,
}

impl TickDriverHandle {
  /// Creates a new driver handle.
  #[must_use]
  pub fn new(
    id: TickDriverId,
    kind: TickDriverKind,
    resolution: Duration,
    control: ArcShared<dyn TickDriverControl>,
  ) -> Self {
    Self { id, kind, resolution, control }
  }

  /// Returns the associated driver identifier.
  #[must_use]
  pub const fn id(&self) -> TickDriverId {
    self.id
  }

  /// Returns the driver classification kind.
  #[must_use]
  pub const fn kind(&self) -> TickDriverKind {
    self.kind
  }

  /// Returns the tick resolution produced by the driver.
  #[must_use]
  pub const fn resolution(&self) -> Duration {
    self.resolution
  }

  /// Stops the underlying driver.
  pub fn shutdown(&self) {
    self.control.shutdown();
  }
}
