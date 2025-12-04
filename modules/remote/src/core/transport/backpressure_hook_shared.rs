//! Shared wrapper for TransportBackpressureHook implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::NoStdMutex,
  sync::{ArcShared, SharedAccess},
};

use super::backpressure_hook::TransportBackpressureHook;

/// Shared wrapper for [`TransportBackpressureHook`] implementations.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying hook, allowing safe
/// concurrent access from multiple owners.
pub struct TransportBackpressureHookShared {
  inner: ArcShared<NoStdMutex<Box<dyn TransportBackpressureHook>>>,
}

impl TransportBackpressureHookShared {
  /// Creates a new shared wrapper around the provided hook.
  #[must_use]
  pub fn new(hook: Box<dyn TransportBackpressureHook>) -> Self {
    Self { inner: ArcShared::new(NoStdMutex::new(hook)) }
  }
}

impl Clone for TransportBackpressureHookShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn TransportBackpressureHook>> for TransportBackpressureHookShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn TransportBackpressureHook>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn TransportBackpressureHook>) -> R) -> R {
    self.inner.with_write(f)
  }
}
