#![cfg(feature = "std")]

extern crate alloc;

use alloc::vec::Vec;
use core::{num::NonZeroUsize, time::Duration};

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorSystem, AnyMessage, AnyMessageView, EventStreamEvent, EventStreamSubscriber,
  LifecycleStage, LogEvent, LogLevel, MailboxConfig, MailboxOverflowStrategy, MailboxPolicy, Props,
};
use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

struct RecordingSubscriber {
  events: SpinSyncMutex<Vec<EventStreamEvent>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: SpinSyncMutex::new(Vec::new()) }
  }

  fn events(&self) -> Vec<EventStreamEvent> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

struct NullActor;

impl Actor for NullActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn event_stream_replays_buffer_for_new_subscribers() {
  let stream = ArcShared::new(cellactor_actor_core_rs::EventStream::default());

  let log = LogEvent::new(LogLevel::Info, alloc::string::String::from("boot"), Duration::from_millis(1), None);
  stream.publish(EventStreamEvent::Log(log));

  let subscriber = ArcShared::new(RecordingSubscriber::new());
  let _subscription = cellactor_actor_core_rs::EventStream::subscribe_arc(&stream, subscriber.clone());

  let lifecycle = cellactor_actor_core_rs::LifecycleEvent::new(
    cellactor_actor_core_rs::Pid::new(1, 0),
    None,
    alloc::string::String::from("actor"),
    LifecycleStage::Started,
    Duration::from_millis(2),
  );
  stream.publish(EventStreamEvent::Lifecycle(lifecycle));

  let events = subscriber.events();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::Log(_))));
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::Lifecycle(_))));
}

#[test]
fn deadletter_is_recorded_when_recipient_is_unavailable() {
  let guardian_props = Props::from_fn(|| NullActor);
  let system = ActorSystem::new(&guardian_props).expect("system");
  let subscriber = ArcShared::new(RecordingSubscriber::new());
  let _subscription = system.subscribe_event_stream(subscriber.clone());

  let mailbox_policy = MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::DropNewest, None);
  let mailbox_config = MailboxConfig::new(mailbox_policy);
  let child = system.spawn(&Props::from_fn(|| NullActor).with_mailbox(mailbox_config)).expect("spawn");
  let actor_ref = child.actor_ref().clone();

  child.suspend().expect("suspend child");
  let err = actor_ref.tell(AnyMessage::new("ping")).expect_err("send should fail while mailbox is suspended");
  assert!(matches!(err, cellactor_actor_core_rs::SendError::Suspended(_)));

  let deadletters = system.deadletters();
  assert!(!deadletters.is_empty());
  assert!(subscriber.events().iter().any(|event| matches!(event, EventStreamEvent::Deadletter(_))));

  child.resume().expect("resume child");
  system.terminate().expect("terminate");
  system.run_until_terminated();
}
