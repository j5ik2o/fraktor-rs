//! Shared wrapper for dynamic `RemoteWatchHook` trait objects.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::{RemoteWatchHook, noop_remote_watch_hook::NoopRemoteWatchHook};
use crate::core::actor::Pid;

/// Shared wrapper that provides thread-safe access to a boxed [`RemoteWatchHook`].
///
/// The hook is wrapped in `ToolboxMutex` and shared via `ArcShared`, while the
/// public surface is limited to `with_read` / `with_write` closures to hide the
/// lock scope and reduce deadlock risk.
///
/// # Design
/// This type follows the `*Shared` pattern from `docs/guides/shared_vs_handle.md`:
/// - Interior mutability is encapsulated within this shared wrapper
/// - The underlying `Box<dyn RemoteWatchHook<TB>>` does not need internal locks
/// - Lock acquisition is hidden from callers via closure-based API
pub(crate) struct RemoteWatchHookDynSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn RemoteWatchHook<TB>>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> RemoteWatchHookDynSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided hook.
  #[must_use]
  pub(crate) fn new(hook: Box<dyn RemoteWatchHook<TB>>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(hook)) }
  }

  /// Creates a new shared wrapper with the default no-op hook.
  #[must_use]
  pub(crate) fn noop() -> Self {
    Self::new(Box::new(NoopRemoteWatchHook))
  }

  /// Acquires a write lock and applies the closure to the inner hook.
  #[inline]
  pub(crate) fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn RemoteWatchHook<TB>>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }

  /// Replaces the current hook with a new one.
  pub(crate) fn replace(&self, hook: Box<dyn RemoteWatchHook<TB>>) {
    self.with_write(|inner| *inner = hook);
  }

  /// Handles a watch request by delegating to the inner hook.
  pub(crate) fn handle_watch(&self, target: Pid, watcher: Pid) -> bool {
    self.with_write(|inner| inner.handle_watch(target, watcher))
  }

  /// Handles an unwatch request by delegating to the inner hook.
  pub(crate) fn handle_unwatch(&self, target: Pid, watcher: Pid) -> bool {
    self.with_write(|inner| inner.handle_unwatch(target, watcher))
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn RemoteWatchHook<TB>>> for RemoteWatchHookDynSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn RemoteWatchHook<TB>>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn RemoteWatchHook<TB>>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for RemoteWatchHookDynSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
