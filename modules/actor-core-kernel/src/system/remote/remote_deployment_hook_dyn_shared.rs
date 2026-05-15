//! Shared wrapper for dynamic `RemoteDeploymentHook` trait objects.

use alloc::boxed::Box;

use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use super::{
  RemoteDeploymentHook, RemoteDeploymentOutcome, RemoteDeploymentRequest,
  noop_remote_deployment_hook::NoopRemoteDeploymentHook,
};

/// Shared wrapper that provides thread-safe access to a boxed [`RemoteDeploymentHook`].
pub(crate) struct RemoteDeploymentHookDynShared {
  inner: SharedLock<ArcShared<dyn RemoteDeploymentHook>>,
}

impl RemoteDeploymentHookDynShared {
  /// Creates a new shared wrapper around the provided hook.
  #[must_use]
  pub(crate) fn new(hook: Box<dyn RemoteDeploymentHook>) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(ArcShared::from_boxed(hook)) }
  }

  /// Creates a shared wrapper with the default no-op hook.
  #[must_use]
  pub(crate) fn noop() -> Self {
    Self::new(Box::new(NoopRemoteDeploymentHook))
  }

  fn current_hook(&self) -> ArcShared<dyn RemoteDeploymentHook> {
    self.inner.with_lock(|inner| inner.clone())
  }

  /// Replaces the current hook with a new one.
  pub(crate) fn replace(&self, hook: Box<dyn RemoteDeploymentHook>) {
    self.inner.with_lock(|inner| *inner = ArcShared::from_boxed(hook));
  }

  /// Delegates a child deployment request to the installed hook.
  pub(crate) fn deploy_child(&self, request: RemoteDeploymentRequest) -> RemoteDeploymentOutcome {
    self.current_hook().deploy_child(request)
  }
}

impl Clone for RemoteDeploymentHookDynShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
