//! Scanning classifier contract.

use core::cmp::Ordering;

use crate::event::bus::EventBus;

/// Contract for buses that scan all subscriptions to find matches.
pub trait ScanningClassification: EventBus {
  /// Provides a total ordering over classifiers.
  #[must_use]
  fn compare_classifiers(&self, left: &Self::Classifier, right: &Self::Classifier) -> Ordering;

  /// Provides a total ordering over subscribers.
  #[must_use]
  fn compare_subscribers(&self, left: &Self::Subscriber, right: &Self::Subscriber) -> Ordering;

  /// Returns whether `classifier` accepts `event`.
  #[must_use]
  fn matches(&self, classifier: &Self::Classifier, event: &Self::Event) -> bool;

  /// Publishes `event` to a matching `subscriber`.
  fn publish_to(&mut self, event: &Self::Event, subscriber: &Self::Subscriber);
}
