//! Shared handle for [`crate::core::extension::Remote`].
//!
//! [`RemoteShared::run`] completes immediately when it is polled after the
//! remote is already terminated. If shutdown is requested while the run future
//! is pending on a receiver, the receiver must still wake the task with an
//! event such as [`crate::core::extension::RemoteEvent::TransportShutdown`].
//! This differs from exclusive [`crate::core::extension::RemoteRunFuture`],
//! which checks termination at the head of its loop whenever it is polled.

use alloc::vec::Vec;

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::core::{
  address::Address,
  association::QuarantineReason,
  envelope::InboundEnvelope,
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
  ///
  /// If the future is polled after the shared remote is already terminated it
  /// completes immediately. If shutdown is requested while a previously polled
  /// future is pending on the receiver, the executor still needs a wake event
  /// (for example [`crate::core::extension::RemoteEvent::TransportShutdown`])
  /// to poll it again, as covered by
  /// `remote_shared_shutdown_without_wake_keeps_run_pending_until_next_event`.
  #[must_use]
  pub const fn run<'a, S: RemoteEventReceiver + ?Sized>(&'a self, receiver: &'a mut S) -> RemoteSharedRunFuture<'a, S> {
    RemoteSharedRunFuture::new(self, receiver)
  }

  /// Drains buffered inbound envelopes observed by the shared core event loop.
  ///
  /// This is a mutating consume operation, not a pure query. The `&self`
  /// signature follows the shared-handle pattern, but the method internally
  /// uses a write lock and delegates to [`Remote::drain_inbound_envelopes`].
  #[must_use]
  pub fn drain_inbound_envelopes(&self) -> Vec<InboundEnvelope> {
    self.with_write(Remote::drain_inbound_envelopes)
  }
}

impl Remoting for RemoteShared {
  fn start(&self) -> Result<(), RemotingError> {
    self.with_write(Remote::start)
  }

  fn shutdown(&self) -> Result<(), RemotingError> {
    self.with_write(|remote| if remote.lifecycle().is_shutdown() { Ok(()) } else { remote.shutdown() })
  }

  fn quarantine(&self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError> {
    self.with_write(|remote| remote.quarantine(address, uid, reason))
  }

  fn addresses(&self) -> Vec<Address> {
    self.with_read(|remote| remote.addresses().to_vec())
  }
}
