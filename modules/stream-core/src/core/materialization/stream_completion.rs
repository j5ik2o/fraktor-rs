use alloc::vec::Vec;
use core::task::Waker;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{Completion, StreamError};

#[cfg(test)]
mod tests;

struct CompletionState<T> {
  result: Option<Result<T, StreamError>>,
  wakers: Vec<Waker>,
}

impl<T> CompletionState<T> {
  const fn new() -> Self {
    Self { result: None, wakers: Vec::new() }
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
        if !guard.wakers.iter().any(|registered| registered.will_wake(waker)) {
          guard.wakers.push(waker.clone());
        }
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

  /// Completes the tracked result if it has not already been resolved.
  pub fn complete(&self, result: Result<T, StreamError>) {
    let wakers = {
      let mut guard = self.inner.lock();
      // 既存結果の上書きを防止
      if guard.result.is_some() {
        return;
      }
      guard.result = Some(result);
      core::mem::take(&mut guard.wakers)
    };
    for waker in wakers {
      waker.wake();
    }
  }
}

impl<T> Default for StreamCompletion<T> {
  fn default() -> Self {
    Self::new()
  }
}
