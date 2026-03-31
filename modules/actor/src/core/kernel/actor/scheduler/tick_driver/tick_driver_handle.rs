//! Handle owning the lifetime of a running tick driver instance.

use alloc::boxed::Box;
use core::{marker::PhantomData, time::Duration};

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use super::{TickDriverControl, TickDriverId, TickDriverKind};

/// Handle owning the lifetime of a running tick driver instance.
pub struct TickDriverHandle {
  id:         TickDriverId,
  kind:       TickDriverKind,
  resolution: Duration,
  control:    ArcShared<RuntimeMutex<Box<dyn TickDriverControl>>>,
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
    control: ArcShared<RuntimeMutex<Box<dyn TickDriverControl>>>,
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

  /// Stops the underlying driver.
  pub fn shutdown(&mut self) {
    self.control.lock().shutdown();
  }
}
