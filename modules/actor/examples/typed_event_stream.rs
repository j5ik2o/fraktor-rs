#![cfg(not(target_os = "none"))]

use core::time::Duration;
use std::vec::Vec;

use fraktor_actor_rs::core::{
  kernel::{
    actor::{
      Pid,
      actor_ref::{ActorRef, ActorRefSender, SendOutcome},
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
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

struct CollectorSender {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl CollectorSender {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl ActorRefSender for CollectorSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if let Some(event) = message.payload().downcast_ref::<EventStreamEvent>() {
      self.events.lock().push(event.clone());
    }
    Ok(SendOutcome::Delivered)
  }
}

fn main() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let collector = ActorRef::new(Pid::new(900, 0), CollectorSender::new(events.clone()));

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
  assert!(
    events
      .lock()
      .iter()
      .any(|event| { matches!(event, EventStreamEvent::Log(log) if log.message() == "typed-event-stream-example") })
  );

  {
    let mut event_stream = system.event_stream();
    event_stream
      .try_tell(EventStreamCommand::Unsubscribe { subscriber: collector.clone() })
      .expect("unsubscribe command should be accepted");
  }

  let mut ignore_ref = system.ignore_ref::<u32>();
  ignore_ref.try_tell(7_u32).expect("ignore_ref should accept messages");

  let system_actor = system
    .system_actor_of(&TypedProps::<u32>::from_behavior_factory(Behaviors::ignore), "typed-event-stream-example")
    .expect("system actor");
  assert_eq!(system_actor.path().expect("system actor path").to_string(), "/system/typed-event-stream-example");
  assert!(system.print_tree().contains("typed-event-stream-example"));

  system.terminate().expect("terminate");
}
