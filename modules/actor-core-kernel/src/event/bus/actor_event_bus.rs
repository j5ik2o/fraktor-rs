//! ActorRef subscriber event bus contract.

use core::cmp::Ordering;

use crate::{actor::actor_ref::ActorRef, event::bus::EventBus};

/// Event bus whose subscribers are actor references.
pub trait ActorEventBus: EventBus<Subscriber = ActorRef> {
  /// Provides the default total ordering used for actor subscribers.
  #[must_use]
  fn compare_actor_subscribers(left: &ActorRef, right: &ActorRef) -> Ordering {
    let left_pid = left.pid();
    let right_pid = right.pid();
    left_pid.value().cmp(&right_pid.value()).then_with(|| left_pid.generation().cmp(&right_pid.generation()))
  }
}

impl<T> ActorEventBus for T where T: EventBus<Subscriber = ActorRef> {}
