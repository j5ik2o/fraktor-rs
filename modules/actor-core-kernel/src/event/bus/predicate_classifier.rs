//! Predicate classifier event bus contract.

use crate::event::bus::EventBus;

/// Marker trait for event buses classified by event predicates.
pub trait PredicateClassifier: EventBus
where
  Self::Classifier: Fn(&Self::Event) -> bool, {
  /// Returns whether `classifier` accepts `event`.
  #[must_use]
  fn matches_predicate(classifier: &Self::Classifier, event: &Self::Event) -> bool {
    classifier(event)
  }
}

impl<T> PredicateClassifier for T
where
  T: EventBus,
  T::Classifier: Fn(&T::Event) -> bool,
{
}
