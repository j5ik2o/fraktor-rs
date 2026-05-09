extern crate alloc;

use alloc::vec::Vec;
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::{
  actor::{
    Pid,
    actor_ref::dead_letter::{DeadLetter, DeadLetterReason, DeadLetterShared},
    error::SendError,
    messaging::AnyMessage,
  },
  event::{
    logging::LogLevel,
    stream::{EventStreamEvent, EventStreamShared, EventStreamSubscriber, tests::subscriber_handle},
  },
};

struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

#[test]
fn record_entry_stores_and_publishes() {
  let stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe(&subscriber);

  let dead_letter = DeadLetterShared::with_default_capacity(stream.clone());
  let pid = Pid::new(1, 0);
  let message = AnyMessage::new("payload");
  dead_letter.record_entry(message, DeadLetterReason::ExplicitRouting, Some(pid), Duration::from_millis(5));

  let entries = dead_letter.entries();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].reason(), DeadLetterReason::ExplicitRouting);
  assert_eq!(entries[0].recipient(), Some(pid));

  let events = events.lock().clone();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::DeadLetter(_))));
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Warn)));
}

#[test]
fn record_send_error_converts_reason_and_honours_capacity() {
  let stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe(&subscriber);

  let deadletter = DeadLetterShared::with_capacity(stream, 1);
  let pid = Pid::new(7, 0);
  let error = SendError::full(AnyMessage::new("first"));
  deadletter.record_send_error(Some(pid), &error, Duration::from_millis(1));
  deadletter.record_entry(
    AnyMessage::new("second"),
    DeadLetterReason::MailboxSuspended,
    Some(pid),
    Duration::from_millis(2),
  );

  let entries = deadletter.entries();
  assert_eq!(entries.len(), 1);
  assert!(matches!(entries[0].reason(), DeadLetterReason::MailboxSuspended));

  let events = events.lock().clone();
  assert!(events.iter().filter(|event| matches!(event, EventStreamEvent::DeadLetter(_))).count() >= 2);
}

#[test]
fn record_send_error_maps_timeout_reason() {
  let stream = EventStreamShared::default();
  let deadletter = DeadLetterShared::with_default_capacity(stream);
  let pid = Pid::new(11, 0);
  let error = SendError::timeout(AnyMessage::new("delayed"));

  deadletter.record_send_error(Some(pid), &error, Duration::from_millis(3));

  let entries = deadletter.entries();
  assert!(
    entries.iter().any(|entry| entry.recipient() == Some(pid) && entry.reason() == DeadLetterReason::MailboxTimeout)
  );
}

#[test]
fn dead_letter_reason_supports_suppressed_and_dropped() {
  let stream = EventStreamShared::default();
  let dead_letter = DeadLetterShared::with_default_capacity(stream);
  let pid = Pid::new(12, 0);

  dead_letter.record_entry(
    AnyMessage::new("suppressed"),
    DeadLetterReason::SuppressedDeadLetter,
    Some(pid),
    Duration::from_millis(1),
  );
  dead_letter.record_entry(AnyMessage::new("dropped"), DeadLetterReason::Dropped, Some(pid), Duration::from_millis(2));

  let entries = dead_letter.entries();
  assert!(entries.iter().any(|entry| entry.reason() == DeadLetterReason::SuppressedDeadLetter));
  assert!(entries.iter().any(|entry| entry.reason() == DeadLetterReason::Dropped));
}

#[test]
fn dead_letter_capacity_drops_oldest_entry() {
  let mut dead_letter = DeadLetter::with_capacity(2);
  assert_eq!(dead_letter.capacity(), 2);

  let first =
    dead_letter.record_entry(AnyMessage::new("one"), DeadLetterReason::ExplicitRouting, None, Duration::from_millis(1));
  let second =
    dead_letter.record_entry(AnyMessage::new("two"), DeadLetterReason::Dropped, None, Duration::from_millis(2));
  let third = dead_letter.record_entry(
    AnyMessage::new("three"),
    DeadLetterReason::SuppressedDeadLetter,
    None,
    Duration::from_millis(3),
  );
  assert_eq!(first.reason(), DeadLetterReason::ExplicitRouting);
  assert_eq!(second.reason(), DeadLetterReason::Dropped);
  assert_eq!(third.reason(), DeadLetterReason::SuppressedDeadLetter);

  let snapshot = dead_letter.snapshot();
  assert_eq!(snapshot.len(), 2);
  assert_eq!(snapshot[0].reason(), DeadLetterReason::Dropped);
  assert_eq!(snapshot[1].reason(), DeadLetterReason::SuppressedDeadLetter);
}

#[test]
fn record_send_error_maps_all_public_send_error_reasons() {
  let mut dead_letter = DeadLetter::with_capacity(8);
  let pid = Pid::new(31, 0);
  let cases = [
    (SendError::suspended(AnyMessage::new("suspended")), DeadLetterReason::MailboxSuspended),
    (SendError::closed(AnyMessage::new("closed")), DeadLetterReason::RecipientUnavailable),
    (SendError::no_recipient(AnyMessage::new("missing")), DeadLetterReason::MissingRecipient),
    (SendError::invalid_payload(AnyMessage::new("invalid"), "expected u32"), DeadLetterReason::SerializationError),
  ];

  for (index, (error, expected)) in cases.into_iter().enumerate() {
    let entry = dead_letter.record_send_error(Some(pid), &error, Duration::from_millis(index as u64));
    assert_eq!(entry.reason(), expected);
    assert_eq!(entry.recipient(), Some(pid));
  }
}
