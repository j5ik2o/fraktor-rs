use alloc::vec::Vec;

use fraktor_actor_adaptor_std_rs::system::new_empty_actor_system;
use fraktor_actor_core_kernel_rs::{
  event::stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscription, subscriber_handle},
  system::ActorSystem,
};
use fraktor_remote_core_rs::extension::EventPublisher;
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedLock};

pub(crate) struct EventHarness {
  system:        ActorSystem,
  publisher:     EventPublisher,
  events:        SharedLock<Vec<EventStreamEvent>>,
  _subscription: EventStreamSubscription,
}

impl EventHarness {
  pub(crate) fn new() -> Self {
    let system = new_empty_actor_system();
    let events = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new());
    let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
    let subscription = system.subscribe_event_stream(&subscriber);
    let publisher = EventPublisher::new(system.downgrade());
    Self { system, publisher, events, _subscription: subscription }
  }

  pub(crate) const fn publisher(&self) -> &EventPublisher {
    &self.publisher
  }

  pub(crate) const fn system(&self) -> &ActorSystem {
    &self.system
  }

  pub(crate) fn events(&self) -> Vec<EventStreamEvent> {
    self.events.with_lock(|events| events.clone())
  }

  pub(crate) fn events_with<R>(&self, f: impl FnOnce(&[EventStreamEvent]) -> R) -> R {
    self.events.with_lock(|events| f(events.as_slice()))
  }
}

struct RecordingSubscriber {
  events: SharedLock<Vec<EventStreamEvent>>,
}

impl RecordingSubscriber {
  fn new(events: SharedLock<Vec<EventStreamEvent>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.with_lock(|events| events.push(event.clone()));
  }
}
