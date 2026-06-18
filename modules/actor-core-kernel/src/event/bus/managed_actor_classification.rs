//! Managed actor-to-actor classifier contract.

use crate::{
  actor::actor_ref::ActorRef,
  event::bus::{ActorClassifier, ActorEventBus},
};

/// Contract for buses where actors subscribe to events classified by actors.
///
/// Implementations usually track actor-to-actor associations and remove every
/// association for an actor when that actor terminates.
pub trait ManagedActorClassification: ActorEventBus + ActorClassifier {
  /// Returns the expected number of distinct actor classifiers.
  #[must_use]
  fn map_size(&self) -> usize;

  /// Returns the actor classifier associated with `event`.
  #[must_use]
  fn classify(&self, event: &Self::Event) -> ActorRef;

  /// Associates `monitor` with `monitored`.
  ///
  /// Returns `true` when the association was added.
  #[must_use]
  fn associate(&mut self, monitored: ActorRef, monitor: ActorRef) -> bool;

  /// Removes every association where `actor` participates.
  fn dissociate(&mut self, actor: &ActorRef);

  /// Removes the association from `monitored` to `monitor`.
  ///
  /// Returns `true` when an existing association was removed.
  #[must_use]
  fn dissociate_pair(&mut self, monitored: &ActorRef, monitor: &ActorRef) -> bool;

  /// Registers `subscriber` with the implementation's termination cleanup path.
  ///
  /// The default is a no-op for implementations without automatic unsubscription.
  #[must_use]
  fn register_with_unsubscriber(&mut self, _subscriber: &ActorRef, _sequence_number: u64) -> bool {
    true
  }

  /// Unregisters `subscriber` from the implementation's termination cleanup path.
  ///
  /// The default is a no-op for implementations without automatic unsubscription.
  #[must_use]
  fn unregister_from_unsubscriber(&mut self, _subscriber: &ActorRef, _sequence_number: u64) -> bool {
    true
  }
}
