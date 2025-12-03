//! Shared wrapper for actor future.

use core::task::Waker;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::ActorFuture;

/// Shared wrapper for [`ActorFuture`] with external mutex synchronization.
///
/// This type provides thread-safe shared access to an `ActorFuture` by wrapping
/// it in `ArcShared<ToolboxMutex<...>>`. This is a thin wrapper that delegates
/// all operations to the inner type by acquiring a lock and calling the
/// corresponding method on [`ActorFuture`].
pub struct ActorFutureSharedGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static, {
  inner: ArcShared<ToolboxMutex<ActorFuture<T, TB>, TB>>,
}

impl<T, TB> Clone for ActorFutureSharedGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T, TB> ActorFutureSharedGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new shared future wrapped in `ArcShared<ToolboxMutex<...>>`.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(ActorFuture::new())) }
  }

  /// Returns the inner `ArcShared<ToolboxMutex<...>>` reference.
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<ToolboxMutex<ActorFuture<T, TB>, TB>> {
    &self.inner
  }

  /// Completes the future with the given value.
  ///
  /// Acquires a lock and delegates to [`ActorFuture::complete`].
  /// The `&self` signature is intentional as the mutex provides interior mutability.
  pub fn complete(&self, value: T) -> Option<Waker> {
    self.inner.lock().complete(value)
  }

  /// Attempts to take the completed value if ready.
  ///
  /// Acquires a lock and delegates to [`ActorFuture::try_take`].
  /// The `&self` signature is intentional as the mutex provides interior mutability.
  #[must_use]
  pub fn try_take(&self) -> Option<T> {
    self.inner.lock().try_take()
  }

  /// Returns whether the future has completed.
  ///
  /// Acquires a lock and delegates to [`ActorFuture::is_ready`].
  #[must_use]
  pub fn is_ready(&self) -> bool {
    self.inner.lock().is_ready()
  }

  /// Registers a waker to be notified when the future completes.
  ///
  /// Acquires a lock and delegates to [`ActorFuture::register_waker`].
  /// The `&self` signature is intentional as the mutex provides interior mutability.
  pub fn register_waker(&self, waker: &Waker) {
    self.inner.lock().register_waker(waker);
  }
}

impl<T, TB> Default for ActorFutureSharedGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}

/// Type alias with the default `NoStdToolbox`.
pub type ActorFutureShared<T> = ActorFutureSharedGeneric<T, NoStdToolbox>;
