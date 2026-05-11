use alloc::{borrow::Cow, boxed::Box, string::String};
use core::any::Any;

use super::Attributes;
use crate::attributes::{
  AsyncBoundaryAttr, Attribute, CancellationStrategyKind, DispatcherAttribute, InputBuffer, LogLevel, LogLevels,
  MandatoryAttribute, Name, SourceLocation,
};

#[test]
fn named_creates_single_name_attribute() {
  let attributes = Attributes::named("stage-a");
  assert_eq!(attributes.names(), &[String::from("stage-a")]);
}

#[test]
fn and_appends_names() {
  let attributes = Attributes::named("left").and(Attributes::named("right"));
  assert_eq!(attributes.names(), &[String::from("left"), String::from("right")]);
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
  impl Attribute for UnrelatedAttr {
    fn as_any(&self) -> &dyn Any {
      self
    }

    fn clone_box(&self) -> Box<dyn Attribute> {
      Box::new(self.clone())
    }

    fn eq_attr(&self, _other: &dyn Any) -> bool {
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
  let result = attributes.get::<AsyncBoundaryAttr>();

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
  let result = attributes.get::<DispatcherAttribute>();

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
  assert!(attributes.get::<AsyncBoundaryAttr>().is_some());
  assert!(attributes.get::<DispatcherAttribute>().is_some());
  assert_eq!(attributes.get::<DispatcherAttribute>().unwrap().name(), "custom-dispatcher");
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
  assert!(cloned.get::<AsyncBoundaryAttr>().is_some());
}

#[test]
fn clone_preserves_dispatcher_attr() {
  // Given: attributes with dispatcher
  let original = Attributes::dispatcher("my-dispatcher");

  // When: cloning
  let cloned = original.clone();

  // Then: dispatcher attribute is preserved
  let dispatcher = cloned.get::<DispatcherAttribute>();
  assert!(dispatcher.is_some());
  assert_eq!(dispatcher.unwrap().name(), "my-dispatcher");
}

// --- contains<T>() / get_all<T>() / cancellation_strategy() tests ---
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
    let attributes = Attributes::cancellation_strategy(CancellationStrategyKind::CompleteStage);

    // Then: the attributes are not empty
    assert!(!attributes.is_empty());
  }

  #[test]
  fn cancellation_strategy_factory_contains_strategy_attr() {
    // Given: attributes created with cancellation_strategy factory
    let attributes = Attributes::cancellation_strategy(CancellationStrategyKind::FailStage);

    // When: requesting the CancellationStrategyKind attribute
    let result = attributes.get::<CancellationStrategyKind>();

    // Then: the attribute is present with correct value
    assert!(result.is_some());
    assert_eq!(*result.unwrap(), CancellationStrategyKind::FailStage);
  }

  #[test]
  fn cancellation_strategy_propagate_failure_variant() {
    // Given: attributes with PropagateFailure strategy
    let attributes = Attributes::cancellation_strategy(CancellationStrategyKind::PropagateFailure);

    // Then: contains the correct strategy
    assert!(attributes.contains::<CancellationStrategyKind>());
    assert_eq!(*attributes.get::<CancellationStrategyKind>().unwrap(), CancellationStrategyKind::PropagateFailure);
  }
} // mod pending_attributes_api

// --- Name attribute (Pekko parity: Attributes.Name) ---

#[test]
fn name_is_constructible_with_string_payload() {
  // Given/When: building a Name attribute directly
  //   Pekko reference: final case class Name(n: String) extends Attribute
  let name = Name(String::from("stage-a"));

  // Then: the inner string is exposed via the public field
  assert_eq!(name.0, "stage-a");
}

#[test]
fn name_clone_preserves_payload() {
  // Given: a Name attribute
  let original = Name(String::from("stage-a"));

  // When: cloning
  let cloned = original.clone();

  // Then: the cloned Name carries the same payload
  assert_eq!(cloned.0, "stage-a");
}

#[test]
fn name_implements_attribute_trait_with_typed_eq() {
  // Given: two Names with identical payloads and an unrelated payload
  let a: Box<dyn Attribute> = Box::new(Name(String::from("foo")));
  let b: Box<dyn Attribute> = Box::new(Name(String::from("foo")));
  let c: Box<dyn Attribute> = Box::new(Name(String::from("bar")));

  // Then: Attribute::eq_attr distinguishes them by payload
  assert!(a.eq_attr(b.as_any()));
  assert!(!a.eq_attr(c.as_any()));
}

#[test]
fn named_factory_also_stores_typed_name_attribute() {
  // Given: attributes constructed via the named() factory
  //   Pekko parity: Attributes(Name(n)) — Name is the canonical typed payload.
  //   fraktor-rs additionally retains the legacy `names: Vec<String>` API.
  let attributes = Attributes::named("stage-a");

  // Then: the typed Name attribute is retrievable via get::<Name>()
  let name = attributes.get::<Name>();
  assert!(name.is_some(), "named() should also store a typed Name attribute");
  assert_eq!(name.unwrap().0, "stage-a");
}

#[test]
fn named_factory_preserves_legacy_names_accessor() {
  // Given: attributes constructed via the named() factory
  let attributes = Attributes::named("stage-a");

  // Then: the legacy names() accessor still returns the configured name
  //   (regression guard — adding a typed Name must not break the existing API)
  assert_eq!(attributes.names(), &[String::from("stage-a")]);
}

#[test]
fn and_appends_typed_name_attributes_for_each_named_call() {
  // Given: two named attribute sets merged together
  let attributes = Attributes::named("left").and(Attributes::named("right"));

  // When: collecting all typed Name attributes
  let names = attributes.get_all::<Name>();

  // Then: both Name instances are preserved in order
  assert_eq!(names.len(), 2);
  assert_eq!(names[0].0, "left");
  assert_eq!(names[1].0, "right");
}

// --- MandatoryAttribute marker trait (Pekko parity: sealed trait MandatoryAttribute) ---

#[test]
fn mandatory_attribute_returns_none_for_empty_attributes() {
  // Given: empty attributes
  let attributes = Attributes::new();

  // When: requesting a mandatory typed attribute
  let result = attributes.mandatory_attribute::<InputBuffer>();

  // Then: returns None
  assert!(result.is_none());
}

#[test]
fn mandatory_attribute_returns_input_buffer_when_stored() {
  // Given: attributes containing an InputBuffer
  //   Pekko reference: InputBuffer extends MandatoryAttribute
  let attributes = Attributes::input_buffer(8, 32);

  // When: requesting via mandatory_attribute helper
  let result = attributes.mandatory_attribute::<InputBuffer>();

  // Then: the InputBuffer is returned with its configured values
  assert!(result.is_some());
  let buffer = result.unwrap();
  assert_eq!(buffer.initial, 8);
  assert_eq!(buffer.max, 32);
}

#[test]
fn mandatory_attribute_returns_dispatcher_attribute_when_stored() {
  // Given: attributes containing a DispatcherAttribute
  //   Pekko reference: Dispatcher extends MandatoryAttribute
  let attributes = Attributes::dispatcher("custom-dispatcher");

  // When: requesting via mandatory_attribute helper
  let result = attributes.mandatory_attribute::<DispatcherAttribute>();

  // Then: the dispatcher is returned with its configured name
  assert!(result.is_some());
  assert_eq!(result.unwrap().name(), "custom-dispatcher");
}

#[test]
fn mandatory_attribute_returns_cancellation_strategy_when_stored() {
  // Given: attributes containing a CancellationStrategyKind
  //   Pekko reference: CancellationStrategy extends MandatoryAttribute
  let attributes = Attributes::cancellation_strategy(CancellationStrategyKind::FailStage);

  // When: requesting via mandatory_attribute helper
  let result = attributes.mandatory_attribute::<CancellationStrategyKind>();

  // Then: the strategy is returned with the configured kind
  assert!(result.is_some());
  assert_eq!(*result.unwrap(), CancellationStrategyKind::FailStage);
}

#[test]
fn mandatory_attribute_marker_is_implemented_for_pekko_parity_set() {
  // Given/When: requiring T: MandatoryAttribute statically for each existing type
  //   This compile-time assertion mirrors Pekko's `extends MandatoryAttribute`
  //   declarations on InputBuffer, Dispatcher, and CancellationStrategy.
  fn assert_mandatory<T: MandatoryAttribute + 'static>() {}
  assert_mandatory::<InputBuffer>();
  assert_mandatory::<DispatcherAttribute>();
  assert_mandatory::<CancellationStrategyKind>();
}

// --- SourceLocation attribute (Pekko parity: Attributes.SourceLocation) ---

#[test]
fn source_location_is_constructible_with_callsite_components() {
  // Given/When: constructing a SourceLocation
  //   Pekko reference: final class SourceLocation(lambda: AnyRef) extends Attribute
  //   Rust translation: callsite components (file, line, column) replace the JVM lambda ref.
  let location = SourceLocation::new(Cow::Borrowed("foo.rs"), 42, 10);

  // Then: the components are recoverable via location_name()
  assert_eq!(location.location_name(), "foo.rs:42:10");
}

#[test]
fn source_location_clone_preserves_components() {
  // Given: a SourceLocation
  let original = SourceLocation::new(Cow::Borrowed("foo.rs"), 42, 10);

  // When: cloning
  let cloned = original.clone();

  // Then: the cloned location renders identically
  assert_eq!(cloned.location_name(), "foo.rs:42:10");
}

#[test]
fn source_location_implements_attribute_trait_with_typed_eq() {
  // Given: two equal SourceLocations and one differing
  let a: Box<dyn Attribute> = Box::new(SourceLocation::new(Cow::Borrowed("foo.rs"), 42, 10));
  let b: Box<dyn Attribute> = Box::new(SourceLocation::new(Cow::Borrowed("foo.rs"), 42, 10));
  let c: Box<dyn Attribute> = Box::new(SourceLocation::new(Cow::Borrowed("foo.rs"), 43, 10));

  // Then: Attribute::eq_attr distinguishes by callsite components
  assert!(a.eq_attr(b.as_any()));
  assert!(!a.eq_attr(c.as_any()));
}

// --- Attributes::source_location() factory ---

#[test]
fn source_location_factory_creates_non_empty_attributes() {
  // Given/When: creating attributes with source_location factory
  let attributes = Attributes::source_location("foo.rs", 42, 10);

  // Then: the attributes are not empty
  assert!(!attributes.is_empty());
}

#[test]
fn source_location_factory_stores_typed_attribute() {
  // Given: attributes created with source_location factory
  let attributes = Attributes::source_location("foo.rs", 42, 10);

  // When: requesting the SourceLocation attribute
  let result = attributes.get::<SourceLocation>();

  // Then: the typed SourceLocation is retrievable with the configured callsite
  assert!(result.is_some());
  assert_eq!(result.unwrap().location_name(), "foo.rs:42:10");
}

#[test]
fn source_location_factory_accepts_owned_string() {
  // Given/When: constructing with an owned String (Into<Cow<'static, str>> path)
  let attributes = Attributes::source_location(String::from("dynamic.rs"), 7, 3);

  // Then: the typed SourceLocation is retrievable
  let location = attributes.get::<SourceLocation>();
  assert!(location.is_some());
  assert_eq!(location.unwrap().location_name(), "dynamic.rs:7:3");
}

#[test]
fn source_location_clone_preserves_typed_attribute() {
  // Given: attributes with a SourceLocation
  let original = Attributes::source_location("foo.rs", 42, 10);

  // When: cloning
  let cloned = original.clone();

  // Then: the typed SourceLocation is preserved
  let location = cloned.get::<SourceLocation>();
  assert!(location.is_some());
  assert_eq!(location.unwrap().location_name(), "foo.rs:42:10");
}
