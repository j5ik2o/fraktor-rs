//! ActorRef classifier event bus contract.

use crate::{actor::actor_ref::ActorRef, event::bus::EventBus};

/// Marker trait for event buses whose classifier is an actor reference.
pub trait ActorClassifier: EventBus<Classifier = ActorRef> {}

impl<T> ActorClassifier for T where T: EventBus<Classifier = ActorRef> {}
