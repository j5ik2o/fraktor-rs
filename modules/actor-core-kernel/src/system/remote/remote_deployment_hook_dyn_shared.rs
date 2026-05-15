//! Shared wrapper for dynamic `RemoteDeploymentHook` trait objects.

use alloc::boxed::Box;

use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::{
  RemoteDeploymentHook, RemoteDeploymentOutcome, RemoteDeploymentRequest,
  noop_remote_deployment_hook::NoopRemoteDeploymentHook,
};

/// Shared wrapper that provides thread-safe access to a boxed [`RemoteDeploymentHook`].
pub(crate) struct RemoteDeploymentHookDynShared {
  inner: SharedLock<Box<dyn RemoteDeploymentHook>>,
}

impl RemoteDeploymentHookDynShared {
  /// Creates a new shared wrapper around the provided hook.
  #[must_use]
  pub(crate) fn new(hook: Box<dyn RemoteDeploymentHook>) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(hook) }
  }

  /// Creates a shared wrapper with the default no-op hook.
  #[must_use]
  pub(crate) fn noop() -> Self {
    Self::new(Box::new(NoopRemoteDeploymentHook))
  }

  /// Acquires a write lock and applies the closure to the inner hook.
  pub(crate) fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn RemoteDeploymentHook>) -> R) -> R {
    self.inner.with_write(f)
  }

  /// Replaces the current hook with a new one.
  pub(crate) fn replace(&self, hook: Box<dyn RemoteDeploymentHook>) {
    self.with_write(|inner| *inner = hook);
  }

  /// Delegates a child deployment request to the installed hook.
  pub(crate) fn deploy_child(&self, request: RemoteDeploymentRequest) -> RemoteDeploymentOutcome {
    self.with_write(|inner| inner.deploy_child(request))
  }
}

impl SharedAccess<Box<dyn RemoteDeploymentHook>> for RemoteDeploymentHookDynShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn RemoteDeploymentHook>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn RemoteDeploymentHook>) -> R) -> R {
    self.inner.with_write(f)
  }
}

impl Clone for RemoteDeploymentHookDynShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
