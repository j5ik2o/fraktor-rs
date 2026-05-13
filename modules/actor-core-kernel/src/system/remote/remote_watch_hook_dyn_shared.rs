//! Shared wrapper for dynamic `RemoteWatchHook` trait objects.

use alloc::boxed::Box;

use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::{RemoteWatchHook, noop_remote_watch_hook::NoopRemoteWatchHook};
use crate::actor::Pid;

/// Shared wrapper that provides thread-safe access to a boxed [`RemoteWatchHook`].
///
/// The hook is wrapped in `SharedLock`, while the
/// public surface is limited to `with_read` / `with_write` closures to hide the
/// lock scope and reduce deadlock risk.
///
/// # Design
/// This type follows the `*Shared` pattern from `docs/guides/shared_vs_handle.md`:
/// - Interior mutability is encapsulated within this shared wrapper
/// - The underlying `Box<dyn RemoteWatchHook>` does not need internal locks
/// - Lock acquisition is hidden from callers via closure-based API
pub(crate) struct RemoteWatchHookDynShared {
  inner: SharedLock<Box<dyn RemoteWatchHook>>,
}

impl RemoteWatchHookDynShared {
  /// Creates a new shared wrapper around the provided hook.
  #[must_use]
  pub(crate) fn new(hook: Box<dyn RemoteWatchHook>) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(hook) }
  }

  /// Creates a new shared wrapper with the default no-op hook.
  #[must_use]
  pub(crate) fn noop() -> Self {
    Self::new(Box::new(NoopRemoteWatchHook))
  }

  /// Acquires a write lock and applies the closure to the inner hook.
  #[inline]
  pub(crate) fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn RemoteWatchHook>) -> R) -> R {
    self.inner.with_write(f)
  }

  /// Replaces the current hook with a new one.
  pub(crate) fn replace(&self, hook: Box<dyn RemoteWatchHook>) {
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

impl SharedAccess<Box<dyn RemoteWatchHook>> for RemoteWatchHookDynShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn RemoteWatchHook>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn RemoteWatchHook>) -> R) -> R {
    self.inner.with_write(f)
  }
}

impl Clone for RemoteWatchHookDynShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
