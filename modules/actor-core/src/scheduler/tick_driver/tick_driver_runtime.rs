//! Runtime assets produced after provisioning a tick driver.

#[cfg(any(test, feature = "test-support"))]
use super::manual_test_driver::ManualTickController;
use super::{TickDriverHandle, TickFeedHandle};
use crate::RuntimeToolbox;

/// Runtime assets produced after provisioning a tick driver.
pub struct TickDriverRuntime<TB: RuntimeToolbox> {
  driver: TickDriverHandle,
  feed:   Option<TickFeedHandle<TB>>,
  #[cfg(any(test, feature = "test-support"))]
  manual: Option<ManualTickController<TB>>,
}

impl<TB: RuntimeToolbox> Clone for TickDriverRuntime<TB> {
  fn clone(&self) -> Self {
    Self {
      driver: self.driver.clone(),
      feed: self.feed.clone(),
      #[cfg(any(test, feature = "test-support"))]
      manual: self.manual.clone(),
    }
  }
}

impl<TB: RuntimeToolbox> TickDriverRuntime<TB> {
  /// Creates a new runtime container for automatic/hardware drivers.
  #[must_use]
  pub const fn new(driver: TickDriverHandle, feed: TickFeedHandle<TB>) -> Self {
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
  pub const fn new_manual(driver: TickDriverHandle, controller: ManualTickController<TB>) -> Self {
    Self { driver, feed: None, manual: Some(controller) }
  }

  /// Returns the driver handle.
  #[must_use]
  pub const fn driver(&self) -> &TickDriverHandle {
    &self.driver
  }

  /// Returns the shared tick feed handle when present.
  #[must_use]
  pub const fn feed(&self) -> Option<&TickFeedHandle<TB>> {
    self.feed.as_ref()
  }

  /// Returns the manual tick controller if available.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub const fn manual_controller(&self) -> Option<&ManualTickController<TB>> {
    self.manual.as_ref()
  }
}
