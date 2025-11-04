//! Deadletter repository publishing notifications to the event stream.

use alloc::{format, vec::Vec};
use core::time::Duration;

use cellactor_utils_core_rs::sync::{ArcShared, SyncMutexFamily, sync_mutex_like::SyncMutexLike};

use crate::{
  NoStdToolbox, RuntimeToolbox, ToolboxMutex,
  actor_prim::Pid,
  dead_letter::{DeadLetterEntry, dead_letter_reason::DeadLetterReason},
  error::SendError,
  event_stream::{EventStreamEvent, EventStreamGeneric},
  logging::{LogEvent, LogLevel},
  messaging::AnyMessage,
};

const DEFAULT_CAPACITY: usize = 256;

/// Collects undeliverable messages and notifies subscribers.
pub struct DeadLetterGeneric<TB: RuntimeToolbox + 'static = NoStdToolbox> {
  entries:      ToolboxMutex<Vec<DeadLetterEntry<TB>>, TB>,
  capacity:     usize,
  event_stream: ArcShared<EventStreamGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> DeadLetterGeneric<TB> {
  /// Creates a new deadletter store with the provided buffer capacity.
  #[must_use]
  pub fn new(event_stream: ArcShared<EventStreamGeneric<TB>>, capacity: usize) -> Self {
    Self { entries: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()), capacity, event_stream }
  }

  /// Creates a new deadletter store with the default capacity.
  #[must_use]
  pub fn with_default_capacity(event_stream: ArcShared<EventStreamGeneric<TB>>) -> Self {
    Self::new(event_stream, DEFAULT_CAPACITY)
  }

  /// Records a send error generated while targeting the specified pid.
  pub fn record_send_error(&self, target: Option<Pid>, error: &SendError<TB>, timestamp: Duration) {
    let reason = match error {
      | SendError::Full(_) => DeadLetterReason::MailboxFull,
      | SendError::Suspended(_) => DeadLetterReason::MailboxSuspended,
      | SendError::Closed(_) => DeadLetterReason::RecipientUnavailable,
      | SendError::NoRecipient(_) => DeadLetterReason::MissingRecipient,
    };
    let message = error.message().clone();
    self.record_entry(message, reason, target, timestamp);
  }

  /// Records an explicit deadletter entry.
  pub fn record_entry(
    &self,
    message: AnyMessage<TB>,
    reason: DeadLetterReason,
    target: Option<Pid>,
    timestamp: Duration,
  ) {
    let entry = DeadLetterEntry::new(message, reason, target, timestamp);
    {
      let mut entries = self.entries.lock();
      entries.push(entry.clone());
      if entries.len() > self.capacity {
        let overflow = entries.len() - self.capacity;
        entries.drain(0..overflow);
      }
    }

    self.publish(&entry);
  }

  /// Returns a snapshot of stored deadletters.
  #[must_use]
  pub fn entries(&self) -> Vec<DeadLetterEntry<TB>> {
    self.entries.lock().clone()
  }

  fn publish(&self, entry: &DeadLetterEntry<TB>) {
    self.event_stream.publish(&EventStreamEvent::DeadLetter(entry.clone()));
    let (origin, message) = match entry.recipient() {
      | Some(pid) => (Some(pid), format!("deadletter for pid {:?} (reason: {:?})", pid, entry.reason())),
      | None => (None, format!("deadletter recorded (reason: {:?})", entry.reason())),
    };
    let log = LogEvent::new(LogLevel::Warn, message, entry.timestamp(), origin);
    self.event_stream.publish(&EventStreamEvent::Log(log));
  }
}

/// Type alias for Deadletter using the default toolbox.
pub type DeadLetter = DeadLetterGeneric<NoStdToolbox>;
