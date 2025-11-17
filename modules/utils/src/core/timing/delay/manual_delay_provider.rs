use alloc::vec::Vec;
use core::time::Duration;

use super::{DelayProvider, delay_future::DelayFuture, delay_trigger::DelayTrigger};
use crate::core::{runtime_toolbox::NoStdMutex, sync::ArcShared};

#[cfg(test)]
mod tests;

/// Manual provider used in tests to deterministically complete delay futures.
#[derive(Clone)]
pub struct ManualDelayProvider {
  handles: ArcShared<NoStdMutex<Vec<DelayTrigger>>>,
}

impl ManualDelayProvider {
  /// Creates a provider without any scheduled delays.
  #[must_use]
  pub fn new() -> Self {
    Self { handles: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  /// Triggers the next pending delay, returning `true` if a future was completed.
  #[must_use]
  pub fn trigger_next(&self) -> bool {
    if let Some(handle) = self.handles.lock().pop() {
      handle.fire();
      true
    } else {
      false
    }
  }

  /// Triggers all pending delays.
  pub fn trigger_all(&self) {
    let mut guard = self.handles.lock();
    for handle in guard.drain(..) {
      handle.fire();
    }
  }

  /// Returns the number of pending handles (testing helper).
  #[must_use]
  pub fn pending_count(&self) -> usize {
    self.handles.lock().len()
  }
}

impl Default for ManualDelayProvider {
  fn default() -> Self {
    Self::new()
  }
}

impl DelayProvider for ManualDelayProvider {
  fn delay(&self, duration: Duration) -> DelayFuture {
    let (future, handle) = DelayFuture::new_pair(duration);
    self.handles.lock().push(handle);
    future
  }
}
