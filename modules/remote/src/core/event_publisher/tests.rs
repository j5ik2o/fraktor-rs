use alloc::vec::Vec;

use fraktor_actor_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::AnyMessageView,
    props::Props,
    scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
    setup::ActorSystemConfig,
  },
  event::stream::{
    BackpressureSignal, CorrelationId, EventStreamEvent, EventStreamSubscriber, EventStreamSubscription,
    RemotingLifecycleEvent, subscriber_handle,
  },
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::EventPublisher;

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| NoopActor).with_name("event-publisher-tests");
  let system_config = ActorSystemConfig::default().with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  ActorSystem::new_with_config(&props, &system_config).expect("system")
}

#[derive(Clone)]
struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

fn subscribe(system: &ActorSystem) -> (RecordingSubscriber, EventStreamSubscription) {
  let recorder = RecordingSubscriber::new();
  let handle = subscriber_handle(recorder.clone());
  let subscription = system.subscribe_event_stream(&handle);
  (recorder, subscription)
}

#[test]
fn publishes_listen_started_event() {
  let system = build_system();
  let publisher = EventPublisher::new(system.downgrade());
  let (recorder, _subscription) = subscribe(&system);

  let correlation = CorrelationId::from_u128(42);
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
  let publisher = EventPublisher::new(system.downgrade());
  let (recorder, _subscription) = subscribe(&system);

  let correlation = CorrelationId::from_u128(7);
  publisher.publish_backpressure("loopback:9000", BackpressureSignal::Apply, correlation);

  let events = recorder.events.lock().clone();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::RemotingBackpressure(ev) if ev.authority() == "loopback:9000" && ev.correlation_id() == correlation)));
}
