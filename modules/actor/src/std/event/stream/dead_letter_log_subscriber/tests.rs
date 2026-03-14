use core::time::Duration;

use super::DeadLetterLogSubscriber;
use crate::{
  core::{
    dead_letter::{DeadLetterEntry, DeadLetterReason},
    event::stream::EventStreamEvent,
    messaging::AnyMessage,
  },
  std::event::stream::EventStreamSubscriber,
};

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
  let event = EventStreamEvent::Log(crate::core::event::logging::LogEvent::new(
    crate::core::event::logging::LogLevel::Info,
    "test".into(),
    Duration::from_secs(0),
    None,
  ));
  listener.on_event(&event);
}

#[test]
fn default_creates_listener() {
  let _listener = DeadLetterLogSubscriber::default();
}
