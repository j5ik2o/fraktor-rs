use super::Attributes;
use crate::core::{InputBuffer, LogLevel, LogLevels};

#[test]
fn named_creates_single_name_attribute() {
  let attributes = Attributes::named("stage-a");
  assert_eq!(attributes.names(), &[alloc::string::String::from("stage-a")]);
}

#[test]
fn and_appends_names() {
  let attributes = Attributes::named("left").and(Attributes::named("right"));
  assert_eq!(attributes.names(), &[alloc::string::String::from("left"), alloc::string::String::from("right")]);
}

#[test]
fn new_is_empty() {
  let attributes = Attributes::new();
  assert!(attributes.is_empty());
}

// --- get<T>() typed attribute access ---

#[test]
fn get_returns_none_for_empty_attributes() {
  // Given: empty attributes
  let attributes = Attributes::new();

  // When: requesting a typed attribute
  let result = attributes.get::<InputBuffer>();

  // Then: returns None
  assert!(result.is_none());
}

#[test]
fn get_returns_typed_attribute_after_input_buffer_factory() {
  // Given: attributes created with input_buffer factory
  let attributes = Attributes::input_buffer(16, 64);

  // When: requesting the InputBuffer attribute
  let result = attributes.get::<InputBuffer>();

  // Then: returns the stored InputBuffer with correct values
  assert!(result.is_some());
  let buffer = result.unwrap();
  assert_eq!(buffer.initial, 16);
  assert_eq!(buffer.max, 64);
}

#[test]
fn get_returns_none_for_unrelated_type() {
  // Given: attributes containing an InputBuffer
  let attributes = Attributes::input_buffer(8, 32);

  // When: requesting a different attribute type
  #[derive(Debug, Clone)]
  struct UnrelatedAttr;
  impl crate::core::Attribute for UnrelatedAttr {
    fn as_any(&self) -> &dyn core::any::Any {
      self
    }

    fn clone_box(&self) -> alloc::boxed::Box<dyn crate::core::Attribute> {
      alloc::boxed::Box::new(self.clone())
    }

    fn eq_attr(&self, _other: &dyn core::any::Any) -> bool {
      false
    }
  }
  let result = attributes.get::<UnrelatedAttr>();

  // Then: returns None
  assert!(result.is_none());
}

// --- input_buffer factory ---

#[test]
fn input_buffer_factory_creates_named_attributes() {
  // Given/When: creating attributes with input_buffer
  let attributes = Attributes::input_buffer(16, 64);

  // Then: the attributes are not empty
  assert!(!attributes.is_empty());
}

// --- log_levels factory ---

#[test]
fn log_levels_factory_creates_attributes() {
  // Given/When: creating attributes with log_levels
  let attributes = Attributes::log_levels(LogLevel::Debug, LogLevel::Info, LogLevel::Error);

  // Then: the attributes are not empty
  assert!(!attributes.is_empty());
}

// --- and() merges typed attributes ---

#[test]
fn and_merges_typed_attributes_from_both_sources() {
  // Given: two attribute sets with different typed attributes
  let left = Attributes::input_buffer(16, 64);
  let right = Attributes::log_levels(LogLevel::Info, LogLevel::Warning, LogLevel::Error);

  // When: merging them
  let merged = left.and(right);

  // Then: both方の型が取得可能
  let buffer = merged.get::<InputBuffer>();
  assert!(buffer.is_some());
  let log_levels = merged.get::<LogLevels>();
  assert!(log_levels.is_some());
}

// --- clone() preserves typed attributes (regression test for QA-001) ---

#[test]
fn clone_preserves_typed_attributes() {
  // Given: attributes containing a typed InputBuffer
  let original = Attributes::input_buffer(16, 64);

  // When: cloning
  let cloned = original.clone();

  // Then: the typed attribute is preserved
  let buffer = cloned.get::<InputBuffer>();
  assert!(buffer.is_some());
  let buffer = buffer.unwrap();
  assert_eq!(buffer.initial, 16);
  assert_eq!(buffer.max, 64);
}

#[test]
fn clone_preserves_log_levels() {
  // Given: attributes containing LogLevels
  let original = Attributes::log_levels(LogLevel::Debug, LogLevel::Info, LogLevel::Error);

  // When: cloning
  let cloned = original.clone();

  // Then: the LogLevels attribute is preserved
  let levels = cloned.get::<LogLevels>();
  assert!(levels.is_some());
  let levels = levels.unwrap();
  assert_eq!(levels.on_element, LogLevel::Debug);
  assert_eq!(levels.on_finish, LogLevel::Info);
  assert_eq!(levels.on_failure, LogLevel::Error);
}

// --- PartialEq includes typed attributes ---

#[test]
fn partial_eq_considers_typed_attributes() {
  // Given: two attributes with same names but different typed attrs
  let a = Attributes::input_buffer(16, 64);
  let b = Attributes::input_buffer(8, 32);

  // Then: they are not equal
  assert_ne!(a, b);
}

#[test]
fn partial_eq_equal_typed_attributes() {
  // Given: two attributes with identical content
  let a = Attributes::input_buffer(16, 64);
  let b = Attributes::input_buffer(16, 64);

  // Then: they are equal
  assert_eq!(a, b);
}
