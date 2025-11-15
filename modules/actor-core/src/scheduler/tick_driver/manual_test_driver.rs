//! Manual tick driver for testing purposes.

#[cfg(any(test, feature = "test-support"))]
use crate::RuntimeToolbox;

/// Manual tick driver for deterministic testing.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone)]
pub struct ManualTestDriver<TB: RuntimeToolbox> {
  _phantom: core::marker::PhantomData<TB>,
}

#[cfg(any(test, feature = "test-support"))]
impl<TB: RuntimeToolbox> ManualTestDriver<TB> {
  /// Creates a new manual test driver.
  #[must_use]
  pub fn new() -> Self {
    Self { _phantom: core::marker::PhantomData }
  }
}

#[cfg(any(test, feature = "test-support"))]
impl<TB: RuntimeToolbox> Default for ManualTestDriver<TB> {
  fn default() -> Self {
    Self::new()
  }
}
