use super::InputBuffer;
use crate::core::Attribute;

// --- Construction ---

#[test]
fn new_creates_with_given_values() {
  // Given/When: creating an InputBuffer with initial=16, max=64
  let buf = InputBuffer::new(16, 64);

  // Then: fields match
  assert_eq!(buf.initial, 16);
  assert_eq!(buf.max, 64);
}

#[test]
fn new_allows_zero_values() {
  // Given/When: creating with zero initial and max
  let buf = InputBuffer::new(0, 0);

  // Then: fields are zero
  assert_eq!(buf.initial, 0);
  assert_eq!(buf.max, 0);
}

#[test]
fn new_allows_initial_equal_to_max() {
  // Given/When: initial equals max
  let buf = InputBuffer::new(32, 32);

  // Then: both are the same
  assert_eq!(buf.initial, buf.max);
}

// --- Attribute trait ---

#[test]
fn as_any_downcast_succeeds() {
  // Given: an InputBuffer stored as a trait object
  let buf = InputBuffer::new(8, 32);
  let boxed: alloc::boxed::Box<dyn Attribute> = alloc::boxed::Box::new(buf);

  // When: downcasting via as_any
  let downcast = boxed.as_any().downcast_ref::<InputBuffer>();

  // Then: downcast succeeds with correct values
  assert!(downcast.is_some());
  let result = downcast.unwrap();
  assert_eq!(result.initial, 8);
  assert_eq!(result.max, 32);
}

#[test]
fn clone_box_produces_independent_copy() {
  // Given: an InputBuffer
  let buf = InputBuffer::new(16, 64);
  let boxed: alloc::boxed::Box<dyn Attribute> = alloc::boxed::Box::new(buf);

  // When: cloning via clone_box
  let cloned = boxed.clone_box();

  // Then: the clone has the same values
  let result = cloned.as_any().downcast_ref::<InputBuffer>();
  assert!(result.is_some());
  assert_eq!(result.unwrap().initial, 16);
  assert_eq!(result.unwrap().max, 64);
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  // Given: two InputBuffers with the same values
  let a = InputBuffer::new(16, 64);
  let b = InputBuffer::new(16, 64);

  // Then: eq_attr returns true
  assert!(a.eq_attr(&b as &dyn core::any::Any));
}

#[test]
fn eq_attr_returns_false_for_different_values() {
  // Given: two InputBuffers with different values
  let a = InputBuffer::new(16, 64);
  let b = InputBuffer::new(8, 32);

  // Then: eq_attr returns false
  assert!(!a.eq_attr(&b as &dyn core::any::Any));
}

// --- Clone / PartialEq ---

#[test]
fn clone_preserves_values() {
  // Given: an InputBuffer
  let original = InputBuffer::new(16, 64);

  // When: cloning
  let cloned = original.clone();

  // Then: values are preserved
  assert_eq!(cloned, original);
}
