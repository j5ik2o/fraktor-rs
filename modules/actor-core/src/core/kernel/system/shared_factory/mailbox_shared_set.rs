//! Mailbox lock bundle for shared mailbox state.

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

/// Lock bundle used by mailbox compound-op synchronization.
///
/// Holds only the `put_lock` barrier (Pekko `putLock` equivalent).
/// The former `invoker` / `actor` / `instrumentation` fields were write-once
/// and are now stored directly in
/// [`Mailbox`](crate::core::kernel::dispatch::mailbox::Mailbox) as
/// `spin::Once<T>` for lock-free reads on the hot path.
#[derive(Clone)]
pub struct MailboxSharedSet {
  put_lock: MailboxLocked<()>,
}

impl MailboxSharedSet {
  /// Creates a mailbox lock bundle from an already materialized shared lock.
  #[must_use]
  pub(crate) const fn new(put_lock: MailboxLocked<()>) -> Self {
    Self { put_lock }
  }

  pub(crate) fn builtin() -> Self {
    Self::new(MailboxLocked::new_with_driver::<DefaultMutex<()>>(()))
  }

  pub(crate) fn put_lock(&self) -> MailboxLocked<()> {
    self.put_lock.clone()
  }
}

pub(crate) type MailboxLocked<T> = SharedLock<T>;
