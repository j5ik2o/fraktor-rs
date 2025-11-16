//! Tick driver trait definitions and runtime handles.

use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::sync::ArcShared;

use super::{TickDriverError, TickDriverId, TickDriverKind, TickFeedHandle};
#[cfg(any(test, feature = "test-support"))]
use super::manual_test_driver::ManualTickController;
use crate::RuntimeToolbox;

static NEXT_DRIVER_ID: AtomicU64 = AtomicU64::new(1);

/// Allocates a unique identifier for tick drivers.
#[must_use]
pub fn next_tick_driver_id() -> TickDriverId {
  TickDriverId::new(NEXT_DRIVER_ID.fetch_add(1, Ordering::Relaxed))
}

/// Common contract implemented by environment-specific tick drivers.
pub trait TickDriver<TB: RuntimeToolbox>: Send + Sync + 'static {
  /// Unique identifier assigned to the driver instance.
  fn id(&self) -> TickDriverId;
  /// Kind classification for observability purposes.
  fn kind(&self) -> TickDriverKind;
  /// Tick resolution produced by this driver.
  fn resolution(&self) -> Duration;
  /// Starts the driver and returns a handle that can be used to stop it later.
  fn start(&self, feed: TickFeedHandle<TB>) -> Result<TickDriverHandle, TickDriverError>;
}

/// Control hook invoked when the driver needs to stop.
pub trait TickDriverControl: Send + Sync + 'static {
  /// Stops the driver and cleans up associated resources.
  fn shutdown(&self);
}

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

/// Runtime assets produced after provisioning a tick driver.
pub struct TickDriverRuntime<TB: RuntimeToolbox> {
  driver: TickDriverHandle,
  feed:   Option<TickFeedHandle<TB>>,
  #[cfg(any(test, feature = "test-support"))]
  manual: Option<ManualTickController<TB>>,
}

impl<TB: RuntimeToolbox> TickDriverRuntime<TB> {
  /// Creates a new runtime container for automatic/hardware drivers.
  #[must_use]
  pub fn new(driver: TickDriverHandle, feed: TickFeedHandle<TB>) -> Self {
    Self {
      driver,
      feed: Some(feed),
      #[cfg(any(test, feature = "test-support"))]
      manual: None,
    }
  }

  /// Creates a manual-driver runtime.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub fn new_manual(driver: TickDriverHandle, controller: ManualTickController<TB>) -> Self {
    Self { driver, feed: None, manual: Some(controller) }
  }

  /// Returns the driver handle.
  #[must_use]
  pub fn driver(&self) -> &TickDriverHandle {
    &self.driver
  }

  /// Returns the shared tick feed handle when present.
  #[must_use]
  pub fn feed(&self) -> Option<&TickFeedHandle<TB>> {
    self.feed.as_ref()
  }

  /// Returns the manual tick controller if available.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub fn manual_controller(&self) -> Option<&ManualTickController<TB>> {
    self.manual.as_ref()
  }
}
