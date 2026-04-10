//! Shared wrapper for RemoteWatchHook implementations.

use core::marker::PhantomData;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

use super::{super::TerminationSignal, ActorRefProvider, RemoteWatchHook, RemoteWatchHookHandle};
use crate::core::kernel::actor::{
  Pid,
  actor_path::{ActorPath, ActorPathScheme},
  actor_ref::ActorRef,
  error::ActorError,
};

/// Shared wrapper that provides thread-safe access to a provider implementing
/// both [`ActorRefProvider`] and [`RemoteWatchHook`].
///
/// The handle is wrapped in `SharedLock`, while the
/// public surface is limited to `with_read` / `with_write` closures to hide the
/// lock scope and reduce deadlock risk.
///
/// # Usage
/// 1. Create: `RemoteWatchHookShared::new(provider, &[ActorPathScheme::FraktorTcp])`
/// 2. Use clones for `ActorRefProvider` registration
/// 3. Pass the same shared instance for `RemoteWatchHook` registration
pub struct RemoteWatchHookShared<P: Send + 'static> {
  inner:   SharedLock<RemoteWatchHookHandle<P>>,
  _marker: PhantomData<()>,
}

impl<P: Send + 'static> RemoteWatchHookShared<P> {
  /// Creates a new shared wrapper around the provided implementation.
  ///
  /// The `schemes` parameter specifies the actor path schemes supported by
  /// the underlying provider for `ActorRefProvider::supported_schemes()`.
  pub fn new(provider: P, schemes: &'static [ActorPathScheme]) -> Self {
    let handle = RemoteWatchHookHandle::new(provider, schemes);
    Self { inner: SharedLock::new_with_driver::<SpinSyncMutex<_>>(handle), _marker: PhantomData }
  }

  /// Acquires a write lock and applies the closure to the inner handle.
  #[inline]
  pub fn with_write<R>(&self, f: impl FnOnce(&mut RemoteWatchHookHandle<P>) -> R) -> R {
    self.inner.with_write(f)
  }

  /// Acquires a read lock and applies the closure to the inner handle.
  #[inline]
  pub fn with_read<R>(&self, f: impl FnOnce(&RemoteWatchHookHandle<P>) -> R) -> R {
    self.inner.with_read(f)
  }
}

impl<P: Send + 'static> SharedAccess<RemoteWatchHookHandle<P>> for RemoteWatchHookShared<P> {
  fn with_read<R>(&self, f: impl FnOnce(&RemoteWatchHookHandle<P>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut RemoteWatchHookHandle<P>) -> R) -> R {
    self.inner.with_write(f)
  }
}

impl<P: Send + 'static> Clone for RemoteWatchHookShared<P> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
  }
}

impl<P: RemoteWatchHook + Send + 'static> RemoteWatchHook for RemoteWatchHookShared<P> {
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.with_write(|inner| inner.handle_watch(target, watcher))
  }

  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> bool {
    self.with_write(|inner| inner.handle_unwatch(target, watcher))
  }
}

impl<P: ActorRefProvider + RemoteWatchHook + Send + 'static> ActorRefProvider for RemoteWatchHookShared<P> {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.with_read(|inner| inner.supported_schemes())
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.with_write(|inner| inner.actor_ref(path))
  }

  fn termination_signal(&self) -> TerminationSignal {
    self.with_read(|inner| inner.termination_signal())
  }
}
