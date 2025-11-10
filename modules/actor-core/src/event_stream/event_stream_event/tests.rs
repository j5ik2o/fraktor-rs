#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use core::time::Duration;

#[cfg(feature = "alloc")]
use super::EventStreamEvent;
#[cfg(feature = "alloc")]
use crate::{
  NoStdToolbox,
  actor_prim::Pid,
  dead_letter::DeadLetterEntry,
  lifecycle::{LifecycleEvent, LifecycleStage},
  logging::{LogEvent, LogLevel},
  mailbox::MailboxMetricsEvent,
  messaging::AnyMessage,
  serialization::{error_event::SerializationErrorEvent, serializer_id::SerializerId},
};

#[cfg(feature = "alloc")]
#[test]
fn event_stream_event_lifecycle_clone() {
  let lifecycle_event = LifecycleEvent::new(
    Pid::new(1, 0),
    None,
    String::from("test-actor"),
    LifecycleStage::Started,
    Duration::from_secs(0),
  );
  let event = EventStreamEvent::<NoStdToolbox>::Lifecycle(lifecycle_event.clone());
  let cloned = event.clone();
  match (event, cloned) {
    | (EventStreamEvent::Lifecycle(e1), EventStreamEvent::Lifecycle(e2)) => {
      assert_eq!(e1.pid(), e2.pid());
      assert_eq!(e1.stage(), e2.stage());
    },
    | _ => panic!("Expected Lifecycle variants"),
  }
}

#[cfg(feature = "alloc")]
#[test]
fn event_stream_event_dead_letter_clone() {
  use crate::dead_letter::DeadLetterReason;

  let entry = DeadLetterEntry::new(
    AnyMessage::new(42u8),
    DeadLetterReason::RecipientUnavailable,
    Some(Pid::new(1, 0)),
    Duration::from_secs(0),
  );
  let event = EventStreamEvent::<NoStdToolbox>::DeadLetter(entry.clone());
  let cloned = event.clone();
  match (event, cloned) {
    | (EventStreamEvent::DeadLetter(e1), EventStreamEvent::DeadLetter(e2)) => {
      assert_eq!(e1.recipient(), e2.recipient());
    },
    | _ => panic!("Expected Deadletter variants"),
  }
}

#[cfg(feature = "alloc")]
#[test]
fn event_stream_event_log_clone() {
  let log_event = LogEvent::new(LogLevel::Info, String::from("test message"), Duration::from_secs(0), None);
  let event = EventStreamEvent::<NoStdToolbox>::Log(log_event.clone());
  let cloned = event.clone();
  match (event, cloned) {
    | (EventStreamEvent::Log(e1), EventStreamEvent::Log(e2)) => {
      assert_eq!(e1.level(), e2.level());
      assert_eq!(e1.message(), e2.message());
    },
    | _ => panic!("Expected Log variants"),
  }
}

#[cfg(feature = "alloc")]
#[test]
fn event_stream_event_mailbox_clone() {
  let metrics_event = MailboxMetricsEvent::new(Pid::new(1, 0), 10, 0, None, None, Duration::from_secs(0));
  let event = EventStreamEvent::<NoStdToolbox>::Mailbox(metrics_event.clone());
  let cloned = event.clone();
  match (event, cloned) {
    | (EventStreamEvent::Mailbox(e1), EventStreamEvent::Mailbox(e2)) => {
      assert_eq!(e1.pid(), e2.pid());
      assert_eq!(e1.user_len(), e2.user_len());
    },
    | _ => panic!("Expected Mailbox variants"),
  }
}

#[cfg(feature = "alloc")]
#[test]
fn event_stream_event_serialization_clone() {
  let event = SerializationErrorEvent::new(
    "payload",
    Some(SerializerId::from_raw(200)),
    Some("manifest".into()),
    Some(Pid::new(1, 0)),
    Some("pekko://sys@host".into()),
  );
  let original = EventStreamEvent::<NoStdToolbox>::Serialization(event.clone());
  let cloned = original.clone();
  match (original, cloned) {
    | (EventStreamEvent::Serialization(e1), EventStreamEvent::Serialization(e2)) => {
      assert_eq!(e1.type_name(), e2.type_name());
      assert_eq!(e1.serializer_id(), e2.serializer_id());
    },
    | _ => panic!("Expected Serialization variants"),
  }
}

#[cfg(feature = "alloc")]
#[test]
fn event_stream_event_debug() {
  fn assert_debug<T: core::fmt::Debug>(_t: &T) {}
  let lifecycle_event =
    LifecycleEvent::new(Pid::new(1, 0), None, String::from("test"), LifecycleStage::Started, Duration::from_secs(0));
  let event = EventStreamEvent::<NoStdToolbox>::Lifecycle(lifecycle_event);
  assert_debug(&event);
}
