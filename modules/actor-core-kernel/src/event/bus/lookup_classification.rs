//! Equality-based classifier contract.

use core::cmp::Ordering;

use crate::event::bus::EventBus;

/// Contract for buses that map events to one classifier by equality.
pub trait LookupClassification: EventBus {
  /// Returns the expected number of distinct classifiers.
  #[must_use]
  fn map_size(&self) -> usize;

  /// Provides a total ordering over subscribers.
  #[must_use]
  fn compare_subscribers(&self, left: &Self::Subscriber, right: &Self::Subscriber) -> Ordering;

  /// Returns the classifier associated with `event`.
  #[must_use]
  fn classify(&self, event: &Self::Event) -> Self::Classifier;

  /// Publishes `event` to a matching `subscriber`.
  fn publish_to(&mut self, event: &Self::Event, subscriber: &Self::Subscriber);
}
