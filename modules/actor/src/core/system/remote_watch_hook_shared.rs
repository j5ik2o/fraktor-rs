//! Shared wrapper for RemoteWatchHook implementations.

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::{ActorRefProvider, RemoteWatchHook, RemoteWatchHookHandle};
use crate::core::{
  actor::{Pid, actor_path::ActorPathScheme, actor_ref::ActorRefGeneric},
  error::ActorError,
};

/// Shared wrapper that provides thread-safe access to a provider implementing
/// both [`ActorRefProvider`] and [`RemoteWatchHook`].
///
/// The handle is wrapped in `ToolboxMutex` and shared via `ArcShared`, while the
/// public surface is limited to `with_read` / `with_write` closures to hide the
/// lock scope and reduce deadlock risk.
///
/// # Usage
/// 1. Create: `RemoteWatchHookShared::new(provider, &[ActorPathScheme::FraktorTcp])`
/// 2. Use clones for `ActorRefProvider` registration
/// 3. Pass the same shared instance for `RemoteWatchHook` registration
pub struct RemoteWatchHookShared<TB: RuntimeToolbox + 'static, P: Send + 'static> {
  inner: ArcShared<ToolboxMutex<RemoteWatchHookHandle<P>, TB>>,
}

impl<TB: RuntimeToolbox + 'static, P: Send + 'static> RemoteWatchHookShared<TB, P> {
  /// Creates a new shared wrapper around the provided implementation.
  ///
  /// The `schemes` parameter specifies the actor path schemes supported by
  /// the underlying provider for `ActorRefProvider::supported_schemes()`.
  pub fn new(provider: P, schemes: &'static [ActorPathScheme]) -> Self {
    let handle = RemoteWatchHookHandle::new(provider, schemes);
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(handle)) }
  }

  /// Acquires a write lock and applies the closure to the inner handle.
  #[inline]
  pub fn with_write<R>(&self, f: impl FnOnce(&mut RemoteWatchHookHandle<P>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }

  /// Acquires a read lock and applies the closure to the inner handle.
  #[inline]
  pub fn with_read<R>(&self, f: impl FnOnce(&RemoteWatchHookHandle<P>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  /// Returns a reference to the inner shared mutex.
  ///
  /// This method is intended for testing and debugging purposes only.
  #[doc(hidden)]
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<ToolboxMutex<RemoteWatchHookHandle<P>, TB>> {
    &self.inner
  }
}

impl<TB: RuntimeToolbox + 'static, P: Send + 'static> SharedAccess<RemoteWatchHookHandle<P>>
  for RemoteWatchHookShared<TB, P>
{
  fn with_read<R>(&self, f: impl FnOnce(&RemoteWatchHookHandle<P>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut RemoteWatchHookHandle<P>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

impl<TB: RuntimeToolbox + 'static, P: Send + 'static> Clone for RemoteWatchHookShared<TB, P> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static, P: RemoteWatchHook<TB> + Send + 'static> RemoteWatchHook<TB>
  for RemoteWatchHookShared<TB, P>
{
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.with_write(|inner| inner.handle_watch(target, watcher))
  }

  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.with_write(|inner| inner.handle_unwatch(target, watcher))
  }
}

impl<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + RemoteWatchHook<TB> + Send + 'static> ActorRefProvider<TB>
  for RemoteWatchHookShared<TB, P>
{
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.with_read(|inner| inner.supported_schemes())
  }

  fn actor_ref(&mut self, path: crate::core::actor::actor_path::ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
    self.with_write(|inner| inner.actor_ref(path))
  }
}
