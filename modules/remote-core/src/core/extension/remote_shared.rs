//! Shared handle for [`crate::core::extension::Remote`].

use alloc::vec::Vec;

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::core::{
  address::Address,
  association::QuarantineReason,
  extension::{Remote, RemoteEventReceiver, RemoteSharedRunFuture, Remoting, RemotingError},
};

/// Shared wrapper for driving remoting through interior locking.
#[derive(Clone)]
pub struct RemoteShared {
  inner: SharedLock<Remote>,
}

impl RemoteShared {
  /// Creates a shared handle around `remote`.
  #[must_use]
  pub fn new(remote: Remote) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(remote) }
  }

  pub(crate) fn with_read<R>(&self, f: impl FnOnce(&Remote) -> R) -> R {
    self.inner.with_read(f)
  }

  pub(crate) fn with_write<R>(&self, f: impl FnOnce(&mut Remote) -> R) -> R {
    self.inner.with_write(f)
  }

  /// Runs the shared core remote event loop until shutdown is requested.
  #[must_use]
  pub const fn run<'a, S: RemoteEventReceiver + ?Sized>(&'a self, receiver: &'a mut S) -> RemoteSharedRunFuture<'a, S> {
    RemoteSharedRunFuture::new(self, receiver)
  }
}

impl Remoting for RemoteShared {
  fn start(&self) -> Result<(), RemotingError> {
    self.with_write(Remote::start)
  }

  fn shutdown(&self) -> Result<(), RemotingError> {
    self.with_write(|remote| if remote.is_terminated() { Ok(()) } else { remote.shutdown() })
  }

  fn quarantine(&self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError> {
    self.with_write(|remote| remote.quarantine(address, uid, reason))
  }

  fn addresses(&self) -> Vec<Address> {
    self.with_read(|remote| remote.addresses().to_vec())
  }
}
