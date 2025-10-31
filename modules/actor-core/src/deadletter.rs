//! Deadletter store maintaining undelivered messages and publishing notifications.

use alloc::{format, vec::Vec};
use core::time::Duration;

use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::{
  any_message::AnyMessage, deadletter_entry::DeadletterEntry, deadletter_reason::DeadletterReason,
  event_stream::EventStream, event_stream_event::EventStreamEvent, log_event::LogEvent, log_level::LogLevel, pid::Pid,
  send_error::SendError,
};

/// Bounded deadletter store that forwards notifications to the event stream.
pub struct Deadletter {
  entries:      SpinSyncMutex<Vec<DeadletterEntry>>,
  capacity:     usize,
  event_stream: ArcShared<EventStream>,
}

impl Deadletter {
  /// Creates a new deadletter repository.
  #[must_use]
  pub fn new(event_stream: ArcShared<EventStream>, capacity: usize) -> Self {
    Self { entries: SpinSyncMutex::new(Vec::new()), capacity, event_stream }
  }

  /// Records a send error generated while addressing the specified pid.
  pub fn record_send_error(&self, target: Option<Pid>, error: &SendError, timestamp: Duration) {
    let reason = match error {
      | SendError::Full(_) => DeadletterReason::MailboxFull,
      | SendError::Suspended(_) => DeadletterReason::MailboxSuspended,
      | SendError::Closed(_) => DeadletterReason::RecipientUnavailable,
      | SendError::NoRecipient(_) => DeadletterReason::MissingRecipient,
    };
    let message = error.message().clone();
    self.record_entry(message, reason, target, timestamp);
  }

  /// Records an explicit deadletter event.
  pub fn record_entry(&self, message: AnyMessage, reason: DeadletterReason, target: Option<Pid>, timestamp: Duration) {
    let entry = DeadletterEntry::new(message, reason, target, timestamp);
    {
      let mut entries = self.entries.lock();
      entries.push(entry.clone());
      if entries.len() > self.capacity {
        let overflow = entries.len() - self.capacity;
        entries.drain(0..overflow);
      }
    }

    self.publish(entry);
  }

  /// Returns a snapshot of stored entries.
  #[must_use]
  pub fn entries(&self) -> Vec<DeadletterEntry> {
    self.entries.lock().clone()
  }

  fn publish(&self, entry: DeadletterEntry) {
    self.event_stream.publish(EventStreamEvent::Deadletter(entry.clone()));
    let (origin, message) = match entry.recipient() {
      | Some(pid) => (Some(pid), format!("deadletter for pid {:?} (reason: {:?})", pid, entry.reason())),
      | None => (None, format!("deadletter recorded (reason: {:?})", entry.reason())),
    };
    let log = LogEvent::new(LogLevel::Warn, message, entry.timestamp(), origin);
    self.event_stream.publish(EventStreamEvent::Log(log));
  }
}
