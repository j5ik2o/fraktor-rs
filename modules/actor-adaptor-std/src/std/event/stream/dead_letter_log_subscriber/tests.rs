use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    actor_ref::dead_letter::{DeadLetterEntry, DeadLetterReason},
    messaging::AnyMessage,
  },
  event::{
    logging::{LogEvent, LogLevel},
    stream::{EventStreamEvent, EventStreamSubscriber},
  },
};

use super::DeadLetterLogSubscriber;

#[test]
fn listener_handles_dead_letter_event_without_panic() {
  let mut listener = DeadLetterLogSubscriber::new();
  let entry =
    DeadLetterEntry::new(AnyMessage::new(42_u32), DeadLetterReason::RecipientUnavailable, None, Duration::from_secs(1));
  let event = EventStreamEvent::DeadLetter(entry);
  listener.on_event(&event);
}

#[test]
fn listener_ignores_non_dead_letter_events() {
  let mut listener = DeadLetterLogSubscriber::new();
  let event = EventStreamEvent::Log(LogEvent::new(LogLevel::Info, "test".into(), Duration::from_secs(0), None, None));
  listener.on_event(&event);
}

#[test]
fn default_creates_listener() {
  let _listener = DeadLetterLogSubscriber::default();
}
