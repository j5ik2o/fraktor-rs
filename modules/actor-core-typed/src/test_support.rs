#![cfg(test)]

use alloc::{boxed::Box, vec::Vec};

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared},
  },
  event::stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberShared},
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedLock, SpinSyncMutex};

#[must_use]
pub(crate) fn actor_ref_with_sender<T>(pid: Pid, sender: T) -> ActorRef
where
  T: ActorRefSender + 'static, {
  ActorRef::new(pid, ActorRefSenderShared::new(Box::new(sender)))
}

#[must_use]
pub(crate) fn subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  EventStreamSubscriberShared::from_shared_lock(SharedLock::new_with_driver::<SpinSyncMutex<_>>(Box::new(subscriber)))
}

#[must_use]
pub(crate) fn test_tick_driver() -> TestTickDriver {
  TestTickDriver::default()
}

pub(crate) struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  #[must_use]
  pub(crate) fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}
