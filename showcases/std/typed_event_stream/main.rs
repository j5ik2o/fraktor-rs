#![cfg(not(target_os = "none"))]

use core::time::Duration;
use std::vec::Vec;

use fraktor_actor_core_rs::core::{
  kernel::{
    actor::{
      Pid,
      actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
      error::SendError,
      messaging::AnyMessage,
      scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    event::{
      logging::{LogEvent, LogLevel},
      stream::EventStreamEvent,
    },
  },
  typed::{TypedActorSystem, TypedProps, dsl::Behaviors, eventstream::EventStreamCommand},
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

struct CollectorSender {
  events: SharedLock<Vec<EventStreamEvent>>,
}

impl CollectorSender {
  fn new(events: SharedLock<Vec<EventStreamEvent>>) -> Self {
    Self { events }
  }
}

impl ActorRefSender for CollectorSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let Some(event) = message.payload().downcast_ref::<EventStreamEvent>() else {
      return Err(SendError::invalid_payload(message, "expected EventStreamEvent"));
    };
    self.events.with_lock(|events| events.push(event.clone()));
    Ok(SendOutcome::Delivered)
  }
}

fn main() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let collector_sender = ActorRefSenderShared::from_shared_lock(SharedLock::new_with_driver::<
    SpinSyncMutex<Box<dyn ActorRefSender>>,
  >(Box::new(CollectorSender::new(events.clone()))));
  let collector = ActorRef::new(Pid::new(900, 0), collector_sender);

  {
    let mut event_stream = system.event_stream();
    event_stream
      .try_tell(EventStreamCommand::Subscribe { subscriber: collector.clone() })
      .expect("subscribe command should be accepted");
  }

  let event = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "typed-event-stream-example".into(),
    Duration::from_millis(1),
    Some(Pid::new(900, 0)),
    None,
  ));
  {
    let mut event_stream = system.event_stream();
    event_stream.try_tell(EventStreamCommand::Publish(event)).expect("publish command should be accepted");
  }
  assert!(events.with_lock(|events| {
    events
      .iter()
      .any(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "typed-event-stream-example"))
  }));

  {
    let mut event_stream = system.event_stream();
    event_stream
      .try_tell(EventStreamCommand::Unsubscribe { subscriber: collector.clone() })
      .expect("unsubscribe command should be accepted");
  }
  let baseline_after_unsubscribe = events.with_lock(|events| events.len());
  let after_unsubscribe = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "typed-event-stream-after-unsubscribe".into(),
    Duration::from_millis(2),
    Some(Pid::new(900, 0)),
    None,
  ));
  {
    let mut event_stream = system.event_stream();
    event_stream
      .try_tell(EventStreamCommand::Publish(after_unsubscribe))
      .expect("publish after unsubscribe should be accepted");
  }
  assert_eq!(
    events.with_lock(|events| events.len()),
    baseline_after_unsubscribe,
    "collector should not receive events after unsubscribe",
  );

  let mut ignore_ref = system.ignore_ref::<u32>();
  ignore_ref.try_tell(7_u32).expect("ignore_ref should accept messages");

  let system_actor = system
    .system_actor_of(&TypedProps::<u32>::from_behavior_factory(Behaviors::ignore), "typed-event-stream-example")
    .expect("system actor");
  assert_eq!(system_actor.path().expect("system actor path").to_string(), "/system/typed-event-stream-example");
  assert!(system.print_tree().contains("typed-event-stream-example"));

  system.terminate().expect("terminate");
}
