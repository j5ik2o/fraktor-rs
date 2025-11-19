use alloc::vec::Vec;

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  event_stream::{
    BackpressureSignal, EventStreamEvent, EventStreamSubscriber, EventStreamSubscriptionGeneric, RemotingLifecycleEvent,
  },
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemConfig, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::EventPublisher;

struct NoopActor;

impl Actor<NoStdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystemGeneric<NoStdToolbox> {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("event-publisher-tests");
  let system_config = ActorSystemConfig::default().with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  ActorSystemGeneric::new_with_config(&props, &system_config).expect("system")
}

#[derive(Clone)]
struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RecordingSubscriber {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

fn subscribe(
  system: &ActorSystemGeneric<NoStdToolbox>,
) -> (RecordingSubscriber, EventStreamSubscriptionGeneric<NoStdToolbox>) {
  let recorder = RecordingSubscriber::new();
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = ArcShared::new(recorder.clone());
  let subscription = system.subscribe_event_stream(&subscriber);
  (recorder, subscription)
}

#[test]
fn publishes_listen_started_event() {
  let system = build_system();
  let publisher = EventPublisher::new(system.clone());
  let (recorder, _subscription) = subscribe(&system);

  let correlation = fraktor_actor_rs::core::event_stream::CorrelationId::from_u128(42);
  publisher.publish_listen_started("127.0.0.1:2552", correlation);

  let events = recorder.events.lock().clone();
  assert!(events.iter().any(|event| {
    matches!(
      event,
      EventStreamEvent::RemotingLifecycle(RemotingLifecycleEvent::ListenStarted { authority, correlation_id })
      if authority == "127.0.0.1:2552" && correlation_id == &correlation
    )
  }));
}

#[test]
fn publishes_backpressure_event() {
  let system = build_system();
  let publisher = EventPublisher::new(system.clone());
  let (recorder, _subscription) = subscribe(&system);

  let correlation = fraktor_actor_rs::core::event_stream::CorrelationId::from_u128(7);
  publisher.publish_backpressure("loopback:9000", BackpressureSignal::Apply, correlation);

  let events = recorder.events.lock().clone();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::RemotingBackpressure(ev) if ev.authority() == "loopback:9000" && ev.correlation_id() == correlation)));
}
