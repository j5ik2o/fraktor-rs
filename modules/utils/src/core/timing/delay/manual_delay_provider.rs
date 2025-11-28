use alloc::vec::Vec;
use core::time::Duration;

use super::{DelayProvider, delay_future::DelayFuture, delay_trigger::DelayTrigger};

#[cfg(test)]
mod tests;

/// Manual provider used in tests to deterministically complete delay futures.
///
/// # Interior Mutability Removed
///
/// This implementation no longer uses interior mutability. All mutating methods
/// now require `&mut self`. If shared access is needed, wrap in an external
/// synchronization primitive (e.g., `Mutex<ManualDelayProvider>`).
pub struct ManualDelayProvider {
  handles: Vec<DelayTrigger>,
}

impl ManualDelayProvider {
  /// Creates a provider without any scheduled delays.
  #[must_use]
  pub const fn new() -> Self {
    Self { handles: Vec::new() }
  }

  /// Triggers the next pending delay, returning `true` if a future was completed.
  #[must_use]
  pub fn trigger_next(&mut self) -> bool {
    if let Some(handle) = self.handles.pop() {
      handle.fire();
      true
    } else {
      false
    }
  }

  /// Triggers all pending delays.
  pub fn trigger_all(&mut self) {
    for handle in self.handles.drain(..) {
      handle.fire();
    }
  }

  /// Returns the number of pending handles (testing helper).
  #[must_use]
  pub fn pending_count(&self) -> usize {
    self.handles.len()
  }
}

impl Default for ManualDelayProvider {
  fn default() -> Self {
    Self::new()
  }
}

impl DelayProvider for ManualDelayProvider {
  fn delay(&mut self, duration: Duration) -> DelayFuture {
    let (future, handle) = DelayFuture::new_pair(duration);
    self.handles.push(handle);
    future
  }
}
