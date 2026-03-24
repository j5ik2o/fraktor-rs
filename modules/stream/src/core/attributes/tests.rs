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

// --- async_boundary() factory ---

#[test]
fn async_boundary_factory_creates_non_empty_attributes() {
  // Given/When: creating attributes with async_boundary factory
  let attributes = Attributes::async_boundary();

  // Then: the attributes are not empty
  assert!(!attributes.is_empty());
}

#[test]
fn async_boundary_factory_contains_async_boundary_attr() {
  // Given: attributes created with async_boundary factory
  let attributes = Attributes::async_boundary();

  // When: requesting the AsyncBoundaryAttr
  let result = attributes.get::<crate::core::AsyncBoundaryAttr>();

  // Then: the attribute is present
  assert!(result.is_some());
}

// --- is_async() detection ---

#[test]
fn is_async_returns_false_for_empty_attributes() {
  // Given: empty attributes
  let attributes = Attributes::new();

  // Then: is_async returns false
  assert!(!attributes.is_async());
}

#[test]
fn is_async_returns_true_for_async_boundary() {
  // Given: attributes with async boundary
  let attributes = Attributes::async_boundary();

  // Then: is_async returns true
  assert!(attributes.is_async());
}

#[test]
fn is_async_returns_true_for_dispatcher_attribute() {
  // Given: attributes with a dispatcher (dispatcher implies async)
  let attributes = Attributes::dispatcher("my-dispatcher");

  // Then: is_async returns true (dispatcher implies async boundary)
  assert!(attributes.is_async());
}

#[test]
fn is_async_returns_false_for_input_buffer_only() {
  // Given: attributes with only input buffer (no async marker)
  let attributes = Attributes::input_buffer(16, 64);

  // Then: is_async returns false
  assert!(!attributes.is_async());
}

#[test]
fn is_async_returns_true_for_async_boundary_and_dispatcher() {
  // Given: attributes combining async boundary and dispatcher
  let attributes = Attributes::async_boundary().and(Attributes::dispatcher("custom-dispatcher"));

  // Then: is_async returns true
  assert!(attributes.is_async());
}

#[test]
fn is_async_returns_true_for_merged_with_async_boundary() {
  // Given: named attributes merged with async boundary
  let attributes = Attributes::named("stage-a").and(Attributes::async_boundary());

  // Then: is_async returns true
  assert!(attributes.is_async());
}

// --- dispatcher() factory ---

#[test]
fn dispatcher_factory_creates_non_empty_attributes() {
  // Given/When: creating attributes with dispatcher factory
  let attributes = Attributes::dispatcher("my-dispatcher");

  // Then: the attributes are not empty
  assert!(!attributes.is_empty());
}

#[test]
fn dispatcher_factory_contains_dispatcher_attr() {
  // Given: attributes created with dispatcher factory
  let attributes = Attributes::dispatcher("my-dispatcher");

  // When: requesting the DispatcherAttribute
  let result = attributes.get::<crate::core::DispatcherAttribute>();

  // Then: the attribute is present with correct name
  assert!(result.is_some());
  assert_eq!(result.unwrap().name(), "my-dispatcher");
}

// --- async_boundary + dispatcher + input_buffer composition ---

#[test]
fn async_boundary_composes_with_dispatcher_and_input_buffer() {
  // Given: Pekko-style composition: async(dispatcher, bufferSize)
  let attributes =
    Attributes::async_boundary().and(Attributes::dispatcher("custom-dispatcher")).and(Attributes::input_buffer(32, 32));

  // Then: all three attributes are retrievable
  assert!(attributes.get::<crate::core::AsyncBoundaryAttr>().is_some());
  assert!(attributes.get::<crate::core::DispatcherAttribute>().is_some());
  assert_eq!(attributes.get::<crate::core::DispatcherAttribute>().unwrap().name(), "custom-dispatcher");
  let buffer = attributes.get::<InputBuffer>();
  assert!(buffer.is_some());
  assert_eq!(buffer.unwrap().initial, 32);
  assert_eq!(buffer.unwrap().max, 32);

  // And: is_async returns true
  assert!(attributes.is_async());
}

// --- clone preserves async attributes ---

#[test]
fn clone_preserves_async_boundary_attr() {
  // Given: attributes with async boundary
  let original = Attributes::async_boundary();

  // When: cloning
  let cloned = original.clone();

  // Then: async boundary attribute is preserved
  assert!(cloned.is_async());
  assert!(cloned.get::<crate::core::AsyncBoundaryAttr>().is_some());
}

#[test]
fn clone_preserves_dispatcher_attr() {
  // Given: attributes with dispatcher
  let original = Attributes::dispatcher("my-dispatcher");

  // When: cloning
  let cloned = original.clone();

  // Then: dispatcher attribute is preserved
  let dispatcher = cloned.get::<crate::core::DispatcherAttribute>();
  assert!(dispatcher.is_some());
  assert_eq!(dispatcher.unwrap().name(), "my-dispatcher");
}

// --- contains<T>() / get_all<T>() / cancellation_strategy() tests ---
// TODO: Attributes::contains, get_all, cancellation_strategy が未実装のため一時的にゲート
#[cfg(any())]
mod pending_attributes_api {
  use super::*;

  #[test]
  fn contains_returns_false_for_empty_attributes() {
    // Given: empty attributes
    let attributes = Attributes::new();

    // Then: contains returns false for any type
    assert!(!attributes.contains::<InputBuffer>());
  }

  #[test]
  fn contains_returns_true_for_stored_type() {
    // Given: attributes with an InputBuffer
    let attributes = Attributes::input_buffer(16, 64);

    // Then: contains returns true for InputBuffer
    assert!(attributes.contains::<InputBuffer>());
  }

  #[test]
  fn contains_returns_false_for_unrelated_type() {
    // Given: attributes with an InputBuffer
    let attributes = Attributes::input_buffer(16, 64);

    // Then: contains returns false for LogLevels
    assert!(!attributes.contains::<LogLevels>());
  }

  // --- get_all<T>() tests ---

  #[test]
  fn get_all_returns_empty_for_no_matches() {
    // Given: empty attributes
    let attributes = Attributes::new();

    // When: requesting all InputBuffer attributes
    let result = attributes.get_all::<InputBuffer>();

    // Then: empty vec
    assert!(result.is_empty());
  }

  #[test]
  fn get_all_returns_single_match() {
    // Given: attributes with one InputBuffer
    let attributes = Attributes::input_buffer(16, 64);

    // When: requesting all InputBuffer attributes
    let result = attributes.get_all::<InputBuffer>();

    // Then: exactly one match
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].initial, 16);
    assert_eq!(result[0].max, 64);
  }

  #[test]
  fn get_all_returns_multiple_matches_from_merged_attributes() {
    // Given: two attribute sets each with InputBuffer merged together
    let merged = Attributes::input_buffer(8, 16).and(Attributes::input_buffer(32, 64));

    // When: requesting all InputBuffer attributes
    let result = merged.get_all::<InputBuffer>();

    // Then: both InputBuffer instances are returned
    assert_eq!(result.len(), 2);
  }

  // --- cancellation_strategy() factory ---

  #[test]
  fn cancellation_strategy_factory_creates_non_empty_attributes() {
    // Given/When: creating attributes with cancellation_strategy factory
    let attributes = Attributes::cancellation_strategy(crate::core::CancellationStrategyKind::CompleteStage);

    // Then: the attributes are not empty
    assert!(!attributes.is_empty());
  }

  #[test]
  fn cancellation_strategy_factory_contains_strategy_attr() {
    // Given: attributes created with cancellation_strategy factory
    let attributes = Attributes::cancellation_strategy(crate::core::CancellationStrategyKind::FailStage);

    // When: requesting the CancellationStrategyKind attribute
    let result = attributes.get::<crate::core::CancellationStrategyKind>();

    // Then: the attribute is present with correct value
    assert!(result.is_some());
    assert_eq!(*result.unwrap(), crate::core::CancellationStrategyKind::FailStage);
  }

  #[test]
  fn cancellation_strategy_propagate_failure_variant() {
    // Given: attributes with PropagateFailure strategy
    let attributes = Attributes::cancellation_strategy(crate::core::CancellationStrategyKind::PropagateFailure);

    // Then: contains the correct strategy
    assert!(attributes.contains::<crate::core::CancellationStrategyKind>());
    assert_eq!(
      *attributes.get::<crate::core::CancellationStrategyKind>().unwrap(),
      crate::core::CancellationStrategyKind::PropagateFailure
    );
  }
} // mod pending_attributes_api
