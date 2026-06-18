//! Subchannel classifier contract.

use crate::event::bus::EventBus;

/// Contract for buses where classifier hierarchy controls delivery.
///
/// A subscriber registered for a parent classifier also receives events from
/// matching child classifiers.
pub trait SubchannelClassification: EventBus {
  /// Returns whether two classifiers are identical for this hierarchy.
  #[must_use]
  fn is_same_classifier(&self, left: &Self::Classifier, right: &Self::Classifier) -> bool;

  /// Returns whether `child` is a sub-classifier of `parent`.
  #[must_use]
  fn is_subclass(&self, child: &Self::Classifier, parent: &Self::Classifier) -> bool;

  /// Returns the classifier associated with `event`.
  #[must_use]
  fn classify(&self, event: &Self::Event) -> Self::Classifier;

  /// Publishes `event` to a matching `subscriber`.
  fn publish_to(&mut self, event: &Self::Event, subscriber: &Self::Subscriber);
}
