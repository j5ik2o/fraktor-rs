//! Shared wrapper for `PathIdentity`.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
};

use super::path_identity::PathIdentity;

/// Thread-safe shared wrapper for [`PathIdentity`].
pub(crate) struct PathIdentitySharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<PathIdentity, TB>>,
}

#[allow(dead_code)]
pub(crate) type PathIdentityShared = PathIdentitySharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> PathIdentitySharedGeneric<TB> {
  pub(crate) fn new(identity: PathIdentity) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(identity)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for PathIdentitySharedGeneric<TB> {
  fn default() -> Self {
    Self::new(PathIdentity::default())
  }
}

impl<TB: RuntimeToolbox> Clone for PathIdentitySharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<PathIdentity> for PathIdentitySharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&PathIdentity) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut PathIdentity) -> R) -> R {
    self.inner.with_write(f)
  }
}
