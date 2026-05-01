use alloc::vec::Vec;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, Waker},
};

use fraktor_actor_core_rs::core::kernel::system::Blocker;
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

/// Future-like handle returned by `Sink` materialization.
///
/// Mirrors Pekko's `Sink[T, Future[T]]` materialized value: a one-shot
/// receiver for the stream's terminal result. Implements [`Future`] for
/// `.await` consumption and exposes [`value`](Self::value) /
/// [`try_take`](Self::try_take) for synchronous polling. Multiple clones
/// observe the same completion result.
pub struct StreamFuture<T> {
  inner: ArcShared<SpinSyncMutex<CompletionState<T>>>,
}

impl<T> Clone for StreamFuture<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T> StreamFuture<T> {
  /// Creates a new pending future handle.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(CompletionState::new())) }
  }

  /// Returns the current completion state without consuming it.
  ///
  /// Equivalent to Pekko's `Future#value`. Returns
  /// [`Completion::Pending`] until the stream resolves.
  #[must_use]
  pub fn value(&self) -> Completion<T>
  where
    T: Clone, {
    let guard = self.inner.lock();
    match guard.result.clone() {
      | Some(result) => Completion::Ready(result),
      | None => Completion::Pending,
    }
  }

  /// Returns `true` once the future has resolved.
  ///
  /// Lock-free of `T: Clone` because it only inspects the presence of a
  /// stored result, not its value. Suitable as the predicate for
  /// [`Blocker::block_until`].
  #[must_use]
  pub fn is_ready(&self) -> bool {
    self.inner.lock().result.is_some()
  }

  /// Blocks the current thread until the future resolves and returns the
  /// result.
  ///
  /// Mirrors [`TerminationSignal::wait_blocking`] for sink-side awaiting:
  /// callers without an async runtime can drive completion via a [`Blocker`]
  /// implementation (`SpinBlocker` for `no_std`, `StdBlocker` for parking).
  ///
  /// Use [`Future::poll`] / `.await` instead when an async runtime is
  /// available.
  ///
  /// # Errors
  ///
  /// Returns the [`StreamError`] reported by the underlying stream when it
  /// terminated abnormally. Returns [`StreamError::StreamDetached`] if the
  /// result was consumed via [`try_take`](Self::try_take) between the
  /// blocker exit and the result read (a misuse pattern).
  ///
  /// [`TerminationSignal::wait_blocking`]:
  /// fraktor_actor_core_rs::core::kernel::system::TerminationSignal::wait_blocking
  pub fn wait_blocking(&self, blocker: &dyn Blocker) -> Result<T, StreamError>
  where
    T: Clone, {
    blocker.block_until(&|| self.is_ready());
    self.inner.lock().result.clone().unwrap_or(Err(StreamError::StreamDetached))
  }

  /// Attempts to take the completion result destructively.
  ///
  /// Subsequent calls to [`value`](Self::value) and [`Future::poll`] return
  /// [`Completion::Pending`] / [`Poll::Pending`] until the stream completes
  /// again, which never happens because completion is one-shot. Use only in
  /// synchronous contexts where ownership of the result is required.
  #[must_use]
  pub fn try_take(&self) -> Option<Result<T, StreamError>> {
    let mut guard = self.inner.lock();
    guard.result.take()
  }

  /// Resolves the future with the given result.
  ///
  /// Idempotent: calls after the first resolution are silently ignored so
  /// the first reported outcome is preserved. All registered wakers are
  /// notified.
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

impl<T> Default for StreamFuture<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> Future for StreamFuture<T>
where
  T: Clone,
{
  type Output = Result<T, StreamError>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let mut guard = self.inner.lock();
    if let Some(result) = guard.result.clone() {
      return Poll::Ready(result);
    }
    if !guard.wakers.iter().any(|registered| registered.will_wake(cx.waker())) {
      guard.wakers.push(cx.waker().clone());
    }
    Poll::Pending
  }
}
