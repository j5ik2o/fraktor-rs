use super::LogLevels;
use crate::core::{Attribute, LogLevel};

// --- Construction ---

#[test]
fn new_creates_with_given_levels() {
  // Given/When: creating LogLevels with specific levels
  let levels = LogLevels::new(LogLevel::Debug, LogLevel::Info, LogLevel::Error);

  // Then: each accessor returns the correct level
  assert_eq!(levels.on_element, LogLevel::Debug);
  assert_eq!(levels.on_finish, LogLevel::Info);
  assert_eq!(levels.on_failure, LogLevel::Error);
}

#[test]
fn new_allows_all_same_level() {
  // Given/When: creating with all levels set to Off
  let levels = LogLevels::new(LogLevel::Off, LogLevel::Off, LogLevel::Off);

  // Then: all are Off
  assert_eq!(levels.on_element, LogLevel::Off);
  assert_eq!(levels.on_finish, LogLevel::Off);
  assert_eq!(levels.on_failure, LogLevel::Off);
}

#[test]
fn new_allows_all_distinct_levels() {
  // Given/When: each lifecycle event has a different level
  let levels = LogLevels::new(LogLevel::Warning, LogLevel::Debug, LogLevel::Error);

  // Then: each is independently stored
  assert_eq!(levels.on_element, LogLevel::Warning);
  assert_eq!(levels.on_finish, LogLevel::Debug);
  assert_eq!(levels.on_failure, LogLevel::Error);
}

// --- Attribute trait ---

#[test]
fn as_any_downcast_succeeds() {
  // Given: a LogLevels stored as a trait object
  let levels = LogLevels::new(LogLevel::Info, LogLevel::Warning, LogLevel::Error);
  let boxed: alloc::boxed::Box<dyn Attribute> = alloc::boxed::Box::new(levels);

  // When: downcasting via as_any
  let downcast = boxed.as_any().downcast_ref::<LogLevels>();

  // Then: downcast succeeds with correct values
  assert!(downcast.is_some());
  let result = downcast.unwrap();
  assert_eq!(result.on_element, LogLevel::Info);
  assert_eq!(result.on_finish, LogLevel::Warning);
  assert_eq!(result.on_failure, LogLevel::Error);
}

#[test]
fn clone_box_produces_independent_copy() {
  // Given: a LogLevels
  let levels = LogLevels::new(LogLevel::Debug, LogLevel::Info, LogLevel::Error);
  let boxed: alloc::boxed::Box<dyn Attribute> = alloc::boxed::Box::new(levels);

  // When: cloning via clone_box
  let cloned = boxed.clone_box();

  // Then: the clone has the same values
  let result = cloned.as_any().downcast_ref::<LogLevels>();
  assert!(result.is_some());
  assert_eq!(result.unwrap().on_element, LogLevel::Debug);
  assert_eq!(result.unwrap().on_finish, LogLevel::Info);
  assert_eq!(result.unwrap().on_failure, LogLevel::Error);
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  // Given: two LogLevels with the same values
  let a = LogLevels::new(LogLevel::Info, LogLevel::Warning, LogLevel::Error);
  let b = LogLevels::new(LogLevel::Info, LogLevel::Warning, LogLevel::Error);

  // Then: eq_attr returns true
  assert!(a.eq_attr(&b as &dyn core::any::Any));
}

#[test]
fn eq_attr_returns_false_for_different_values() {
  // Given: two LogLevels with different values
  let a = LogLevels::new(LogLevel::Info, LogLevel::Warning, LogLevel::Error);
  let b = LogLevels::new(LogLevel::Debug, LogLevel::Info, LogLevel::Error);

  // Then: eq_attr returns false
  assert!(!a.eq_attr(&b as &dyn core::any::Any));
}

// --- Copy / Clone / PartialEq ---

#[test]
fn copy_preserves_values() {
  // Given: a LogLevels
  let original = LogLevels::new(LogLevel::Debug, LogLevel::Info, LogLevel::Error);

  // When: copying
  let copied = original;

  // Then: values are preserved
  assert_eq!(copied, original);
}
