//! Shared wrapper for [`DeadLetter`] with deadlock-safe event publishing.
//!
//! This module provides thread-safe access to [`DeadLetter`] while ensuring
//! that event stream notifications are executed without holding the deadletter lock,
//! preventing potential deadlocks.

use alloc::{format, vec::Vec};
use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeRwLock,
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use crate::core::{
  actor::Pid,
  dead_letter::{DeadLetter, DeadLetterEntry, DeadLetterReason},
  error::SendError,
  event::{
    logging::{LogEvent, LogLevel},
    stream::{EventStreamEvent, EventStreamShared},
  },
  messaging::AnyMessage,
};

const DEFAULT_CAPACITY: usize = 256;

/// Shared wrapper that provides thread-safe access to [`DeadLetter`].
///
/// This type implements the hybrid locking pattern to avoid deadlocks:
/// - Lock acquisition is minimized to data mutation only
/// - Event stream notifications are executed after releasing the lock
///
/// # Design
///
/// The key insight is separating "data mutation" from "event publishing":
///
/// ```text
/// record_entry():
///   1. Acquire write lock
///   2. Store entry, get entry for notification
///   3. Release lock  <-- Lock released BEFORE publishing
///   4. Publish events to event stream (no lock held)
/// ```
pub struct DeadLetterShared {
  inner:        ArcShared<RuntimeRwLock<DeadLetter>>,
  event_stream: EventStreamShared,
}

impl DeadLetterShared {
  /// Creates a shared deadletter store with the specified capacity.
  #[must_use]
  pub fn with_capacity(event_stream: EventStreamShared, capacity: usize) -> Self {
    Self { inner: ArcShared::new(RuntimeRwLock::new(DeadLetter::with_capacity(capacity))), event_stream }
  }

  /// Creates a shared deadletter store with the default capacity.
  #[must_use]
  pub fn with_default_capacity(event_stream: EventStreamShared) -> Self {
    Self::with_capacity(event_stream, DEFAULT_CAPACITY)
  }

  /// Records a send error generated while targeting the specified pid.
  ///
  /// Event stream notifications are sent after releasing the lock to prevent deadlocks.
  pub fn record_send_error(&self, target: Option<Pid>, error: &SendError, timestamp: Duration) {
    // Phase 1: Acquire lock, record entry, release lock
    let entry = {
      let mut guard = self.inner.write();
      guard.record_send_error(target, error, timestamp)
    };
    // Lock released here!

    // Phase 2: Publish events without holding the lock
    self.publish(&entry);
  }

  /// Records an explicit deadletter entry.
  ///
  /// Event stream notifications are sent after releasing the lock to prevent deadlocks.
  pub fn record_entry(&self, message: AnyMessage, reason: DeadLetterReason, target: Option<Pid>, timestamp: Duration) {
    // Phase 1: Acquire lock, record entry, release lock
    let entry = {
      let mut guard = self.inner.write();
      guard.record_entry(message, reason, target, timestamp)
    };
    // Lock released here!

    // Phase 2: Publish events without holding the lock
    self.publish(&entry);
  }

  /// Returns a snapshot of stored deadletters.
  #[must_use]
  pub fn entries(&self) -> Vec<DeadLetterEntry> {
    self.inner.read().snapshot()
  }

  fn publish(&self, entry: &DeadLetterEntry) {
    self.event_stream.publish(&EventStreamEvent::DeadLetter(entry.clone()));
    let (origin, message) = match entry.recipient() {
      | Some(pid) => (Some(pid), format!("deadletter for pid {:?} (reason: {:?})", pid, entry.reason())),
      | None => (None, format!("deadletter recorded (reason: {:?})", entry.reason())),
    };
    let log = LogEvent::new(LogLevel::Warn, message, entry.timestamp(), origin);
    self.event_stream.publish(&EventStreamEvent::Log(log));
  }
}

impl Clone for DeadLetterShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), event_stream: self.event_stream.clone() }
  }
}

impl PartialEq for DeadLetterShared {
  fn eq(&self, other: &Self) -> bool {
    ArcShared::ptr_eq(&self.inner, &other.inner)
  }
}

impl Eq for DeadLetterShared {}

impl SharedAccess<DeadLetter> for DeadLetterShared {
  fn with_read<R>(&self, f: impl FnOnce(&DeadLetter) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut DeadLetter) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
