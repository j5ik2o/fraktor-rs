use fraktor_actor_core_rs::actor::{
  Pid,
  actor_ref::{ActorRefSender, SendOutcome},
  error::SendError,
  messaging::AnyMessage,
};

use crate::eventstream::EventStreamCommand;

struct StubSender;

impl ActorRefSender for StubSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}

// --- Phase 1 タスク5: Subscribe / Unsubscribe variants ---

/// `EventStreamCommand::Subscribe` can be constructed with an actor reference.
#[test]
fn subscribe_variant_holds_actor_ref() {
  let subscriber = crate::test_support::actor_ref_with_sender(Pid::new(10, 0), StubSender);
  let pid = subscriber.pid();

  let command = EventStreamCommand::Subscribe { subscriber };

  match command {
    | EventStreamCommand::Subscribe { subscriber } => {
      assert_eq!(subscriber.pid(), pid, "subscriber should retain the correct pid");
    },
    | _ => panic!("expected Subscribe variant"),
  }
}

/// `EventStreamCommand::Unsubscribe` can be constructed with an actor reference.
#[test]
fn unsubscribe_variant_holds_actor_ref() {
  let subscriber = crate::test_support::actor_ref_with_sender(Pid::new(20, 0), StubSender);
  let pid = subscriber.pid();

  let command = EventStreamCommand::Unsubscribe { subscriber };

  match command {
    | EventStreamCommand::Unsubscribe { subscriber } => {
      assert_eq!(subscriber.pid(), pid, "subscriber should retain the correct pid");
    },
    | _ => panic!("expected Unsubscribe variant"),
  }
}

/// All three variants of `EventStreamCommand` are distinguishable via pattern matching.
#[test]
fn all_variants_are_distinguishable() {
  use alloc::string::ToString;
  use core::time::Duration;

  use fraktor_actor_core_rs::{
    actor::lifecycle::{LifecycleEvent, LifecycleStage},
    event::stream::EventStreamEvent,
  };

  let lifecycle_event =
    LifecycleEvent::new(Pid::new(1, 0), None, "test".to_string(), LifecycleStage::Started, Duration::ZERO);
  let publish = EventStreamCommand::Publish(EventStreamEvent::Lifecycle(lifecycle_event));
  let subscribe = EventStreamCommand::Subscribe {
    subscriber: crate::test_support::actor_ref_with_sender(Pid::new(2, 0), StubSender),
  };
  let unsubscribe = EventStreamCommand::Unsubscribe {
    subscriber: crate::test_support::actor_ref_with_sender(Pid::new(3, 0), StubSender),
  };

  assert!(matches!(publish, EventStreamCommand::Publish(_)));
  assert!(matches!(subscribe, EventStreamCommand::Subscribe { .. }));
  assert!(matches!(unsubscribe, EventStreamCommand::Unsubscribe { .. }));
}
