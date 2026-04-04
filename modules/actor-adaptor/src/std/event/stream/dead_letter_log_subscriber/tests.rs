use core::time::Duration;

use fraktor_actor_rs::core::kernel::{
  actor::{
    actor_ref::dead_letter::{DeadLetterEntry, DeadLetterReason},
    messaging::AnyMessage,
  },
  event::stream::EventStreamEvent,
};

use super::DeadLetterLogSubscriber;
use crate::std::event::stream::EventStreamSubscriber;

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
  let event = EventStreamEvent::Log(fraktor_actor_rs::core::kernel::event::logging::LogEvent::new(
    fraktor_actor_rs::core::kernel::event::logging::LogLevel::Info,
    "test".into(),
    Duration::from_secs(0),
    None,
    None,
  ));
  listener.on_event(&event);
}

#[test]
fn default_creates_listener() {
  let _listener = DeadLetterLogSubscriber::default();
}
