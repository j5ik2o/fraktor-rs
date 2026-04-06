use crate::core::CompletionStrategy;

// --- constructibility ---

#[test]
fn completion_strategy_immediately_is_constructible() {
  // Given/When: constructing the Immediately variant
  let strategy = CompletionStrategy::Immediately;

  // Then: it matches the expected variant
  assert!(matches!(strategy, CompletionStrategy::Immediately));
}

#[test]
fn completion_strategy_draining_is_constructible() {
  // Given/When: constructing the Draining variant
  let strategy = CompletionStrategy::Draining;

  // Then: it matches the expected variant
  assert!(matches!(strategy, CompletionStrategy::Draining));
}

// --- equality ---

#[test]
fn completion_strategy_immediately_equals_itself() {
  // Given: two Immediately values
  let a = CompletionStrategy::Immediately;
  let b = CompletionStrategy::Immediately;

  // Then: they are equal
  assert_eq!(a, b);
}

#[test]
fn completion_strategy_draining_equals_itself() {
  // Given: two Draining values
  let a = CompletionStrategy::Draining;
  let b = CompletionStrategy::Draining;

  // Then: they are equal
  assert_eq!(a, b);
}

#[test]
fn completion_strategy_immediately_differs_from_draining() {
  // Given: both variants
  let immediately = CompletionStrategy::Immediately;
  let draining = CompletionStrategy::Draining;

  // Then: they are not equal
  assert_ne!(immediately, draining);
}

// --- Copy semantics ---

#[test]
fn completion_strategy_is_copy() {
  // Given: a CompletionStrategy value
  let original = CompletionStrategy::Immediately;

  // When: copying
  let copied = original;

  // Then: both are usable and equal
  assert_eq!(original, copied);
}

// --- Clone ---

#[test]
fn completion_strategy_clone_preserves_variant() {
  // Given: a Draining strategy
  let original = CompletionStrategy::Draining;

  // When: cloning
  let cloned = original.clone();

  // Then: clone equals original
  assert_eq!(original, cloned);
}
