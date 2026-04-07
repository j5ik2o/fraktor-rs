use alloc::{collections::VecDeque, vec::Vec};

use fraktor_utils_rs::core::sync::{ArcShared, SpinSyncMutex};

#[cfg(test)]
mod tests;

/// Thread-safe callback queue used to hand over asynchronous signals to stage logic.
pub struct AsyncCallback<T> {
  values: ArcShared<SpinSyncMutex<VecDeque<T>>>,
}

impl<T> Clone for AsyncCallback<T> {
  fn clone(&self) -> Self {
    Self { values: self.values.clone() }
  }
}

impl<T> AsyncCallback<T> {
  /// Creates an empty callback queue.
  #[must_use]
  pub fn new() -> Self {
    Self { values: ArcShared::new(SpinSyncMutex::new(VecDeque::new())) }
  }

  /// Enqueues an asynchronous value.
  pub fn invoke(&self, value: T) {
    let mut guard = self.values.lock();
    guard.push_back(value);
  }

  /// Drains all currently queued asynchronous values.
  #[must_use]
  pub fn drain(&self) -> Vec<T> {
    let mut guard = self.values.lock();
    guard.drain(..).collect()
  }

  /// Returns the number of queued values.
  #[must_use]
  pub fn len(&self) -> usize {
    let guard = self.values.lock();
    guard.len()
  }

  /// Returns `true` when there are no queued values.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }
}

impl<T> Default for AsyncCallback<T> {
  fn default() -> Self {
    Self::new()
  }
}
