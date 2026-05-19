use alloc::vec::Vec;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, Waker},
};

use fraktor_actor_core_kernel_rs::system::Blocker;
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::{Completion, StreamError};

#[cfg(test)]
#[path = "stream_future_test.rs"]
mod tests;

struct CompletionState<T> {
  result:    Option<Result<T, StreamError>>,
  /// Sticky flag set on first `complete()` and never cleared.
  ///
  /// Decoupled from `result.is_some()` so that destructive readers like
  /// `try_take` cannot revert observers (e.g. `wait_blocking`) into the
  /// pending state and deadlock.
  completed: bool,
  wakers:    Vec<Waker>,
}

impl<T> CompletionState<T> {
  const fn new() -> Self {
    Self { result: None, completed: false, wakers: Vec::new() }
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
  /// Independent of `T: Clone` because it only inspects the sticky
  /// `completed` flag, not the stored value. The flag is set on the first
  /// [`complete`](Self::complete) and never cleared, so destructive readers
  /// like [`try_take`](Self::try_take) cannot revert observers back into the
  /// pending state. Suitable as the predicate for [`Blocker::block_until`].
  #[must_use]
  pub fn is_ready(&self) -> bool {
    self.inner.lock().completed
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
  /// result was consumed via [`try_take`](Self::try_take) before this
  /// reader could observe it (`is_ready` reports completion via a sticky
  /// flag so the blocker still unblocks in that race, instead of waiting
  /// forever).
  ///
  /// [`TerminationSignal::wait_blocking`]:
  /// fraktor_actor_core_kernel_rs::system::TerminationSignal::wait_blocking
  pub fn wait_blocking(&self, blocker: &dyn Blocker) -> Result<T, StreamError>
  where
    T: Clone, {
    blocker.block_until(&|| self.is_ready());
    self.inner.lock().result.clone().unwrap_or(Err(StreamError::StreamDetached))
  }

  /// Attempts to take the completion result destructively.
  ///
  /// After this call returns [`Some`], the stored result is cleared but the
  /// sticky `completed` flag remains set. Subsequent observers therefore see:
  ///
  /// - [`value`](Self::value) → [`Completion::Pending`] (only `result` is inspected; one-shot
  ///   semantics, so it never re-arrives)
  /// - [`is_ready`](Self::is_ready) → `true` (sticky `completed` flag)
  /// - [`Future::poll`] → [`Poll::Ready`] with
  ///   [`Err(StreamError::StreamDetached)`](StreamError::StreamDetached) so an awaiting clone is
  ///   unblocked instead of hanging forever
  /// - [`wait_blocking`](Self::wait_blocking) → `Err(StreamError::StreamDetached)` for the same
  ///   reason
  ///
  /// Use only in synchronous contexts where ownership of the result is
  /// required and no other clone observes the future.
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
      // sticky な completed フラグで再呼び出しを抑止する
      // (try_take 後の result.is_some() は false に戻るため使えない)
      if guard.completed {
        return;
      }
      guard.completed = true;
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
    if guard.completed {
      // 結果は既に消費済 (try_take) 。`complete()` は sticky な completed
      // フラグで再呼び出しを抑止するため、ここで Pending を返すと永久に
      // wake されない。`wait_blocking` と整合させて StreamDetached を返す。
      return Poll::Ready(Err(StreamError::StreamDetached));
    }
    if !guard.wakers.iter().any(|registered| registered.will_wake(cx.waker())) {
      guard.wakers.push(cx.waker().clone());
    }
    Poll::Pending
  }
}
