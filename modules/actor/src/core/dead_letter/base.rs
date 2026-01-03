//! Deadletter repository for undeliverable messages.

use alloc::vec::Vec;
use core::time::Duration;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::{
  actor::Pid,
  dead_letter::{DeadLetterEntryGeneric, dead_letter_reason::DeadLetterReason},
  error::SendError,
  messaging::AnyMessageGeneric,
};

const DEFAULT_CAPACITY: usize = 256;

/// Collects undeliverable messages.
///
/// This type uses `&mut self` methods for state modification.
/// For shared access, use [`DeadLetterSharedGeneric`].
///
/// [`DeadLetterSharedGeneric`]: super::DeadLetterSharedGeneric
pub struct DeadLetterGeneric<TB: RuntimeToolbox + 'static> {
  entries:  Vec<DeadLetterEntryGeneric<TB>>,
  capacity: usize,
}

impl<TB: RuntimeToolbox + 'static> DeadLetterGeneric<TB> {
  /// Creates a new deadletter store with the provided buffer capacity.
  #[must_use]
  pub const fn with_capacity(capacity: usize) -> Self {
    Self { entries: Vec::new(), capacity }
  }

  /// Records a send error and returns the created entry for notification.
  ///
  /// The caller is responsible for publishing the entry to the event stream
  /// after releasing any locks.
  #[must_use]
  pub fn record_send_error(
    &mut self,
    target: Option<Pid>,
    error: &SendError<TB>,
    timestamp: Duration,
  ) -> DeadLetterEntryGeneric<TB> {
    let reason = match error {
      | SendError::Full(_) => DeadLetterReason::MailboxFull,
      | SendError::Suspended(_) => DeadLetterReason::MailboxSuspended,
      | SendError::Closed(_) => DeadLetterReason::RecipientUnavailable,
      | SendError::NoRecipient(_) => DeadLetterReason::MissingRecipient,
      | SendError::Timeout(_) => DeadLetterReason::MailboxTimeout,
    };
    let message = error.message().clone();
    self.record_entry(message, reason, target, timestamp)
  }

  /// Records an explicit deadletter entry and returns it for notification.
  ///
  /// The caller is responsible for publishing the entry to the event stream
  /// after releasing any locks.
  #[must_use]
  pub fn record_entry(
    &mut self,
    message: AnyMessageGeneric<TB>,
    reason: DeadLetterReason,
    target: Option<Pid>,
    timestamp: Duration,
  ) -> DeadLetterEntryGeneric<TB> {
    let entry = DeadLetterEntryGeneric::new(message, reason, target, timestamp);
    self.entries.push(entry.clone());
    if self.entries.len() > self.capacity {
      let overflow = self.entries.len() - self.capacity;
      self.entries.drain(0..overflow);
    }
    entry
  }

  /// Returns a snapshot of stored deadletters.
  #[must_use]
  pub fn snapshot(&self) -> Vec<DeadLetterEntryGeneric<TB>> {
    self.entries.clone()
  }

  /// Returns the buffer capacity.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.capacity
  }
}

impl<TB: RuntimeToolbox + 'static> Default for DeadLetterGeneric<TB> {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY)
  }
}

/// Type alias for `DeadLetterGeneric` with the default `NoStdToolbox`.
pub type DeadLetter = DeadLetterGeneric<NoStdToolbox>;
