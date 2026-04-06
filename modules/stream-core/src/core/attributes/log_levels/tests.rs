use super::LogLevels;
use crate::core::attributes::{Attribute, LogLevel};

#[test]
fn new_creates_with_given_levels() {
  let levels = LogLevels::new(LogLevel::Debug, LogLevel::Info, LogLevel::Error);
  assert_eq!(levels.on_element, LogLevel::Debug);
  assert_eq!(levels.on_finish, LogLevel::Info);
  assert_eq!(levels.on_failure, LogLevel::Error);
}

#[test]
fn new_allows_all_same_level() {
  let levels = LogLevels::new(LogLevel::Off, LogLevel::Off, LogLevel::Off);
  assert_eq!(levels.on_element, LogLevel::Off);
  assert_eq!(levels.on_finish, LogLevel::Off);
  assert_eq!(levels.on_failure, LogLevel::Off);
}

#[test]
fn new_allows_all_distinct_levels() {
  let levels = LogLevels::new(LogLevel::Warning, LogLevel::Debug, LogLevel::Error);
  assert_eq!(levels.on_element, LogLevel::Warning);
  assert_eq!(levels.on_finish, LogLevel::Debug);
  assert_eq!(levels.on_failure, LogLevel::Error);
}

#[test]
fn as_any_downcast_succeeds() {
  let boxed: alloc::boxed::Box<dyn Attribute> =
    alloc::boxed::Box::new(LogLevels::new(LogLevel::Info, LogLevel::Warning, LogLevel::Error));
  let downcast = boxed.as_any().downcast_ref::<LogLevels>();
  assert!(downcast.is_some());
  let result = downcast.unwrap();
  assert_eq!(result.on_element, LogLevel::Info);
  assert_eq!(result.on_finish, LogLevel::Warning);
  assert_eq!(result.on_failure, LogLevel::Error);
}

#[test]
fn clone_box_produces_independent_copy() {
  let boxed: alloc::boxed::Box<dyn Attribute> =
    alloc::boxed::Box::new(LogLevels::new(LogLevel::Debug, LogLevel::Info, LogLevel::Error));
  let cloned = boxed.clone_box();
  let result = cloned.as_any().downcast_ref::<LogLevels>().unwrap();
  assert_eq!(result.on_element, LogLevel::Debug);
  assert_eq!(result.on_finish, LogLevel::Info);
  assert_eq!(result.on_failure, LogLevel::Error);
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  let lhs = LogLevels::new(LogLevel::Info, LogLevel::Warning, LogLevel::Error);
  let rhs = LogLevels::new(LogLevel::Info, LogLevel::Warning, LogLevel::Error);
  assert!(lhs.eq_attr(&rhs as &dyn core::any::Any));
}

#[test]
fn eq_attr_returns_false_for_different_values() {
  let lhs = LogLevels::new(LogLevel::Info, LogLevel::Warning, LogLevel::Error);
  let rhs = LogLevels::new(LogLevel::Debug, LogLevel::Info, LogLevel::Error);
  assert!(!lhs.eq_attr(&rhs as &dyn core::any::Any));
}

#[test]
fn copy_preserves_values() {
  let original = LogLevels::new(LogLevel::Debug, LogLevel::Info, LogLevel::Error);
  let copied = original;
  assert_eq!(copied, original);
}
