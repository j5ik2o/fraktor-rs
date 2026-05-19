use alloc::string::String;

use crate::{attributes::Attributes, snapshot::LogicSnapshot};

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_preserves_index() {
  // Given: a logic snapshot with index 5
  let logic = LogicSnapshot::new(5, "stage", Attributes::new());

  // Then: index() returns 5
  assert_eq!(logic.index(), 5);
}

#[test]
fn new_preserves_label_from_str_slice() {
  // Given: a label provided as &str
  let logic = LogicSnapshot::new(0, "my-stage", Attributes::new());

  // Then: label() returns the same string
  assert_eq!(logic.label(), "my-stage");
}

#[test]
fn new_preserves_label_from_owned_string() {
  // Given: a label provided as owned String
  let owned = String::from("owned-stage");
  let logic = LogicSnapshot::new(0, owned, Attributes::new());

  // Then: label() returns the same content
  assert_eq!(logic.label(), "owned-stage");
}

#[test]
fn new_preserves_attributes() {
  // Given: attributes carrying a stage name
  let attrs = Attributes::named("decorated");
  let logic = LogicSnapshot::new(0, "stage", attrs);

  // Then: attributes() exposes the stored name collection
  assert_eq!(logic.attributes().names(), &[alloc::string::String::from("decorated")]);
}

// ---------------------------------------------------------------------------
// Boundary values
// ---------------------------------------------------------------------------

#[test]
fn new_accepts_u32_max_index() {
  // Given: the maximum u32 index
  let logic = LogicSnapshot::new(u32::MAX, "edge", Attributes::new());

  // Then: index() returns u32::MAX
  assert_eq!(logic.index(), u32::MAX);
}

#[test]
fn new_accepts_empty_label() {
  // Given: an empty string label
  let logic = LogicSnapshot::new(0, "", Attributes::new());

  // Then: label() returns an empty string
  assert_eq!(logic.label(), "");
}

// ---------------------------------------------------------------------------
// Derive trait verification
// ---------------------------------------------------------------------------

#[test]
fn clone_produces_equal_values() {
  // Given: a logic snapshot
  let original = LogicSnapshot::new(3, "cloned", Attributes::named("tag"));

  // When: cloned
  let cloned = original.clone();

  // Then: every field is preserved
  assert_eq!(cloned.index(), 3);
  assert_eq!(cloned.label(), "cloned");
  assert_eq!(cloned.attributes(), original.attributes());
}

#[test]
fn debug_format_contains_index_and_label() {
  // Given: a logic snapshot with distinctive values
  let logic = LogicSnapshot::new(17, "debuggable", Attributes::new());

  // When: formatted with Debug
  let debug = alloc::format!("{logic:?}");

  // Then: both the index and the label are present in the output
  assert!(debug.contains("17"));
  assert!(debug.contains("debuggable"));
}
