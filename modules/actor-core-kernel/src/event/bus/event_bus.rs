//! Base event bus contract.

/// Base contract for user-defined event buses.
///
/// Implementations choose the event, classifier, and subscriber types, then
/// provide the subscription and publication semantics. This mirrors Pekko's
/// `EventBus` surface while using Rust naming for the all-classifier
/// unsubscribe operation.
pub trait EventBus {
  /// Event type published on the bus.
  type Event;

  /// Classifier type used to select subscribers.
  type Classifier;

  /// Subscriber type registered on the bus.
  type Subscriber;

  /// Attempts to register `subscriber` for `to`.
  ///
  /// Returns `true` when the subscription was added.
  #[must_use]
  fn subscribe(&mut self, subscriber: Self::Subscriber, to: Self::Classifier) -> bool;

  /// Attempts to remove `subscriber` from `from`.
  ///
  /// Returns `true` when an existing subscription was removed.
  #[must_use]
  fn unsubscribe(&mut self, subscriber: &Self::Subscriber, from: &Self::Classifier) -> bool;

  /// Removes `subscriber` from every classifier.
  fn unsubscribe_all(&mut self, subscriber: &Self::Subscriber);

  /// Publishes `event` to matching subscribers.
  fn publish(&mut self, event: Self::Event);
}
