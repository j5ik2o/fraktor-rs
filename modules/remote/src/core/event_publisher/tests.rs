use alloc::vec::Vec;

use fraktor_actor_rs::core::event_stream::{
  BackpressureSignal, EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, RemotingLifecycleEvent,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use super::EventPublisher;

struct RecordingSubscriber {
  events: ToolboxMutex<Vec<EventStreamEvent<NoStdToolbox>>, NoStdToolbox>,
}

impl RecordingSubscriber {
  fn new() -> ArcShared<Self> {
    ArcShared::new(Self {
      events: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()),
    })
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

#[test]
fn listen_started_event_contains_authority_and_correlation_id() {
  let stream = ArcShared::new(EventStreamGeneric::default());
  let subscriber = RecordingSubscriber::new();
  let subscriber_ref: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&stream, &subscriber_ref);
  let publisher = EventPublisher::new(stream);
  let correlation = publisher.next_correlation_id();

  publisher.lifecycle_listen_started("127.0.0.1:2552", correlation);

  assert!(subscriber.events.lock().iter().any(|event| match event {
    | EventStreamEvent::RemotingLifecycle(RemotingLifecycleEvent::ListenStarted { authority, correlation_id }) => {
      authority == "127.0.0.1:2552" && *correlation_id == correlation
    },
    | _ => false,
  }));
}

#[test]
fn backpressure_event_propagates_signal_and_correlation_id() {
  let stream = ArcShared::new(EventStreamGeneric::default());
  let subscriber = RecordingSubscriber::new();
  let subscriber_ref: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&stream, &subscriber_ref);
  let publisher = EventPublisher::new(stream);
  let correlation = publisher.next_correlation_id();

  publisher.backpressure("node-a", BackpressureSignal::Apply, correlation);

  assert!(subscriber.events.lock().iter().any(|event| match event {
    | EventStreamEvent::RemotingBackpressure(backpressure) => {
      backpressure.authority() == "node-a"
        && matches!(backpressure.signal(), BackpressureSignal::Apply)
        && backpressure.correlation_id() == correlation
    },
    | _ => false,
  }));
}
