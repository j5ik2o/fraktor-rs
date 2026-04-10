//! Handle owning the lifetime of a running tick driver instance.

use alloc::boxed::Box;
use core::{marker::PhantomData, time::Duration};

use fraktor_utils_core_rs::core::sync::SharedLock;

use super::{TickDriverControl, TickDriverId, TickDriverKind};

/// Handle owning the lifetime of a running tick driver instance.
pub struct TickDriverHandle {
  id:         TickDriverId,
  kind:       TickDriverKind,
  resolution: Duration,
  control:    SharedLock<Box<dyn TickDriverControl>>,
  _marker:    PhantomData<()>,
}

impl Clone for TickDriverHandle {
  fn clone(&self) -> Self {
    Self {
      id:         self.id,
      kind:       self.kind,
      resolution: self.resolution,
      control:    self.control.clone(),
      _marker:    PhantomData,
    }
  }
}

impl TickDriverHandle {
  /// Creates a new driver handle.
  #[must_use]
  pub fn new(
    id: TickDriverId,
    kind: TickDriverKind,
    resolution: Duration,
    control: SharedLock<Box<dyn TickDriverControl>>,
  ) -> Self {
    Self { id, kind, resolution, control, _marker: PhantomData }
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

  #[must_use]
  pub(crate) fn control(&self) -> SharedLock<Box<dyn TickDriverControl>> {
    self.control.clone()
  }

  /// Stops the underlying driver.
  pub fn shutdown(&mut self) {
    self.control.with_lock(|control| control.shutdown());
  }
}
