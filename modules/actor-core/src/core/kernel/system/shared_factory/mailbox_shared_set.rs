//! Mailbox lock bundle for shared mailbox state.

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

use crate::core::kernel::dispatch::mailbox::MailboxClock;

/// Lock bundle used by mailbox compound-op synchronization.
///
/// Holds the `put_lock` barrier (Pekko `putLock` equivalent) and the optional
/// throughput deadline clock (see [`MailboxClock`]). The former
/// `invoker` / `actor` / `instrumentation` fields were write-once and are now
/// stored directly in [`Mailbox`](crate::core::kernel::dispatch::mailbox::Mailbox)
/// as `SyncOnce<T>` for lock-free reads on the hot path.
///
/// `clock = None` disables throughput deadline enforcement, reducing to the
/// previous throughput-only yield behaviour (Pekko
/// `isThroughputDeadlineTimeDefined = false` equivalent). `no_std` core builds
/// fall back to this default; std adaptors inject a monotonic clock via
/// [`MailboxSharedSet::with_clock`] during `ActorSystem` initialization.
#[derive(Clone)]
pub struct MailboxSharedSet {
  put_lock: MailboxLocked<()>,
  clock:    Option<MailboxClock>,
}

impl MailboxSharedSet {
  /// Creates a mailbox lock bundle from an already materialized shared lock.
  ///
  /// `clock` defaults to `None`; use [`Self::with_clock`] to inject a monotonic
  /// clock source.
  #[must_use]
  pub(crate) fn new(put_lock: MailboxLocked<()>) -> Self {
    Self { put_lock, clock: None }
  }

  pub(crate) fn builtin() -> Self {
    Self::new(MailboxLocked::new_with_driver::<DefaultMutex<()>>(()))
  }

  /// Replaces the installed clock with the provided one.
  ///
  /// Calling this method on a bundle that already has `clock = Some(_)` simply
  /// overwrites the previous value; it never panics. The `ActorSystem`
  /// initialization path calls this once per system, while tests or embedded
  /// adaptors may substitute a different clock before constructing mailboxes.
  #[must_use]
  pub(crate) fn with_clock(mut self, clock: MailboxClock) -> Self {
    self.clock = Some(clock);
    self
  }

  pub(crate) fn put_lock(&self) -> MailboxLocked<()> {
    self.put_lock.clone()
  }

  /// Returns a reference to the installed clock, or `None` when deadline
  /// enforcement is disabled for this bundle.
  pub(crate) fn clock(&self) -> Option<&MailboxClock> {
    self.clock.as_ref()
  }
}

pub(crate) type MailboxLocked<T> = SharedLock<T>;
