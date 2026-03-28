#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use core::time::Duration;

#[cfg(feature = "alloc")]
#[cfg(feature = "alloc")]
use super::EventStreamEvent;
#[cfg(feature = "alloc")]
use crate::core::{
  kernel::actor::Pid,
  kernel::dead_letter::DeadLetterEntry,
  kernel::dispatch::mailbox::metrics_event::MailboxMetricsEvent,
  kernel::event::logging::{LogEvent, LogLevel},
  kernel::event::stream::{AdapterFailureEvent, TypedUnhandledMessageEvent},
  kernel::lifecycle::{LifecycleEvent, LifecycleStage},
  kernel::messaging::AnyMessage,
  kernel::serialization::{SerializationErrorEvent, SerializerId},
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
  let event = EventStreamEvent::Lifecycle(lifecycle_event.clone());
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
  use crate::core::kernel::dead_letter::DeadLetterReason;

  let entry = DeadLetterEntry::new(
    AnyMessage::new(42u8),
    DeadLetterReason::RecipientUnavailable,
    Some(Pid::new(1, 0)),
    Duration::from_secs(0),
  );
  let event = EventStreamEvent::DeadLetter(entry.clone());
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
  let event = EventStreamEvent::Log(log_event.clone());
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
  let event = EventStreamEvent::Mailbox(metrics_event.clone());
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
fn event_stream_event_unhandled_message_clone() {
  let payload = TypedUnhandledMessageEvent::new(Pid::new(1, 0), String::from("probe::Command"), Duration::from_secs(3));
  let event = EventStreamEvent::UnhandledMessage(payload.clone());
  let cloned = event.clone();
  match (event, cloned) {
    | (EventStreamEvent::UnhandledMessage(left), EventStreamEvent::UnhandledMessage(right)) => {
      assert_eq!(left.actor(), right.actor());
      assert_eq!(left.message(), right.message());
      assert_eq!(left.timestamp(), right.timestamp());
    },
    | _ => panic!("Expected UnhandledMessage variants"),
  }
}

#[cfg(feature = "alloc")]
#[test]
fn event_stream_event_adapter_failure_clone() {
  let payload = AdapterFailureEvent::custom(Pid::new(1, 0), String::from("boom"));
  let event = EventStreamEvent::AdapterFailure(payload.clone());
  let cloned = event.clone();
  match (event, cloned) {
    | (EventStreamEvent::AdapterFailure(left), EventStreamEvent::AdapterFailure(right)) => match (left, right) {
      | (
        AdapterFailureEvent::Custom { pid: left_pid, detail: left_detail },
        AdapterFailureEvent::Custom { pid: right_pid, detail: right_detail },
      ) => {
        assert_eq!(left_pid, right_pid);
        assert_eq!(left_detail, right_detail);
      },
      | _ => panic!("Expected custom adapter failure events"),
    },
    | _ => panic!("Expected AdapterFailure variants"),
  }
}

#[cfg(feature = "alloc")]
#[test]
fn event_stream_event_extension_clone() {
  let payload = AnyMessage::new(String::from("cluster-startup"));
  let event = EventStreamEvent::Extension { name: String::from("cluster"), payload: payload.clone() };
  let cloned = event.clone();

  match (event, cloned) {
    | (
      EventStreamEvent::Extension { name: left_name, payload: left_payload },
      EventStreamEvent::Extension { name: right_name, payload: right_payload },
    ) => {
      assert_eq!(left_name, right_name);
      let left_str = left_payload.payload().downcast_ref::<String>().unwrap();
      let right_str = right_payload.payload().downcast_ref::<String>().unwrap();
      assert_eq!(left_str, right_str);
    },
    | _ => panic!("Expected Extension variants"),
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
    Some("fraktor://sys@host".into()),
  );
  let original = EventStreamEvent::Serialization(event.clone());
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
  let event = EventStreamEvent::Lifecycle(lifecycle_event);
  assert_debug(&event);
}
