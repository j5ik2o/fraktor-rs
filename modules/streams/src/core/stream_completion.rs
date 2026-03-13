use core::task::Waker;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{Completion, StreamError};

#[cfg(test)]
mod tests;

struct CompletionState<T> {
  result: Option<Result<T, StreamError>>,
  waker:  Option<Waker>,
}

impl<T> CompletionState<T> {
  const fn new() -> Self {
    Self { result: None, waker: None }
  }
}

/// Handle used to observe stream completion.
pub struct StreamCompletion<T> {
  inner: ArcShared<SpinSyncMutex<CompletionState<T>>>,
}

impl<T> Clone for StreamCompletion<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T> StreamCompletion<T> {
  /// Creates a new completion handle.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(CompletionState::new())) }
  }

  /// Polls the completion state.
  #[must_use]
  pub fn poll(&self) -> Completion<T>
  where
    T: Clone, {
    let guard = self.inner.lock();
    match guard.result.clone() {
      | Some(result) => Completion::Ready(result),
      | None => Completion::Pending,
    }
  }

  pub(crate) fn poll_with_waker(&self, waker: &Waker) -> Completion<T>
  where
    T: Clone, {
    let mut guard = self.inner.lock();
    match guard.result.clone() {
      | Some(result) => Completion::Ready(result),
      | None => {
        guard.waker = Some(waker.clone());
        Completion::Pending
      },
    }
  }

  /// Attempts to take the completion result.
  #[must_use]
  pub fn try_take(&self) -> Option<Result<T, StreamError>> {
    let mut guard = self.inner.lock();
    guard.result.take()
  }

  pub(crate) fn complete(&self, result: Result<T, StreamError>) {
    let waker = {
      let mut guard = self.inner.lock();
      // 既存結果の上書きを防止
      if guard.result.is_some() {
        return;
      }
      guard.result = Some(result);
      guard.waker.take()
    };
    if let Some(waker) = waker {
      waker.wake();
    }
  }
}

impl<T> Default for StreamCompletion<T> {
  fn default() -> Self {
    Self::new()
  }
}
