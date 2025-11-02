#![cfg(feature = "std")]

extern crate alloc;

use alloc::vec::Vec;
use std::{thread, time::Duration};

use cellactor_actor_core_rs::{
  Actor, ActorContext, ActorError, ActorSystem, AnyMessage, AnyMessageView, EventStreamEvent, EventStreamSubscriber,
  LifecycleStage, LogLevel, NoStdToolbox, Props,
};
use cellactor_utils_core_rs::sync::{ArcShared, NoStdMutex};

struct Start;

struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
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

struct Guardian;

impl Actor for Guardian {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_, NoStdToolbox>) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "guardian pre_start");
    Ok(())
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_, NoStdToolbox>,
    message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      ctx.log(LogLevel::Info, "received Start message");
      ctx.stop_self().ok();
    }
    Ok(())
  }
}

#[test]
fn lifecycle_and_log_events_are_published() {
  let props = Props::from_fn(|| Guardian);
  let system = ActorSystem::new(&props).expect("system");

  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn cellactor_actor_core_rs::EventStreamSubscriber> = subscriber_impl.clone();
  let _subscription = system.subscribe_event_stream(&subscriber);

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("send start");

  wait_until(|| {
    let events = subscriber_impl.events();
    events.iter().any(
      |event| matches!(event, EventStreamEvent::Lifecycle(lifecycle) if lifecycle.stage() == LifecycleStage::Started),
    ) && events.iter().any(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "guardian pre_start"))
  });

  system.terminate().expect("terminate");
  system.run_until_terminated();

  wait_until(|| {
    subscriber_impl.events().iter().any(
      |event| matches!(event, EventStreamEvent::Lifecycle(lifecycle) if lifecycle.stage() == LifecycleStage::Stopped),
    )
  });
}

fn wait_until(condition: impl Fn() -> bool) {
  let deadline = std::time::Instant::now() + Duration::from_secs(1);
  while !condition() && std::time::Instant::now() < deadline {
    thread::yield_now();
  }
}
