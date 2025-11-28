//! Runtime assets produced after provisioning a tick driver.

use alloc::boxed::Box;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

#[cfg(any(test, feature = "test-support"))]
use super::manual_test_driver::ManualTickController;
use super::{AutoDriverMetadata, TickDriverHandleGeneric, TickFeedHandle};

/// Runtime assets produced after provisioning a tick driver.
pub struct TickDriverRuntime<TB: RuntimeToolbox> {
  driver:            TickDriverHandleGeneric<TB>,
  feed:              Option<TickFeedHandle<TB>>,
  executor_shutdown: Option<Box<dyn FnOnce() + Send>>,
  auto_metadata:     Option<AutoDriverMetadata>,
  #[cfg(any(test, feature = "test-support"))]
  manual:            Option<ManualTickController<TB>>,
}

impl<TB: RuntimeToolbox> Clone for TickDriverRuntime<TB> {
  fn clone(&self) -> Self {
    Self {
      driver: self.driver.clone(),
      feed: self.feed.clone(),
      executor_shutdown: None, // Executor shutdown is owned by the original instance
      auto_metadata: self.auto_metadata.clone(),
      #[cfg(any(test, feature = "test-support"))]
      manual: self.manual.clone(),
    }
  }
}

impl<TB: RuntimeToolbox> TickDriverRuntime<TB> {
  /// Creates a new runtime container for automatic/hardware drivers.
  #[must_use]
  pub const fn new(driver: TickDriverHandleGeneric<TB>, feed: TickFeedHandle<TB>) -> Self {
    Self {
      driver,
      feed: Some(feed),
      executor_shutdown: None,
      auto_metadata: None,
      #[cfg(any(test, feature = "test-support"))]
      manual: None,
    }
  }

  /// Adds an executor shutdown callback to this runtime.
  #[must_use]
  pub fn with_executor_shutdown<F>(mut self, shutdown: F) -> Self
  where
    F: FnOnce() + Send + 'static, {
    self.executor_shutdown = Some(Box::new(shutdown));
    self
  }

  /// Annotates the runtime with auto driver metadata.
  #[must_use]
  pub const fn with_auto_metadata(mut self, metadata: AutoDriverMetadata) -> Self {
    self.auto_metadata = Some(metadata);
    self
  }

  /// Creates a manual-driver runtime.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub const fn new_manual(driver: TickDriverHandleGeneric<TB>, controller: ManualTickController<TB>) -> Self {
    Self { driver, feed: None, executor_shutdown: None, auto_metadata: None, manual: Some(controller) }
  }

  /// Returns the driver handle.
  #[must_use]
  pub const fn driver(&self) -> &TickDriverHandleGeneric<TB> {
    &self.driver
  }

  /// Returns the shared tick feed handle when present.
  #[must_use]
  pub const fn feed(&self) -> Option<&TickFeedHandle<TB>> {
    self.feed.as_ref()
  }

  /// Returns the auto driver metadata if present.
  #[must_use]
  pub const fn auto_metadata(&self) -> Option<&AutoDriverMetadata> {
    self.auto_metadata.as_ref()
  }

  /// Returns the manual tick controller if available.
  #[cfg(any(test, feature = "test-support"))]
  #[must_use]
  pub const fn manual_controller(&self) -> Option<&ManualTickController<TB>> {
    self.manual.as_ref()
  }
}
