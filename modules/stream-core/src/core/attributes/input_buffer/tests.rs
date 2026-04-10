use alloc::boxed::Box;
use core::any::Any;

use super::InputBuffer;
use crate::core::attributes::Attribute;

#[test]
fn new_creates_with_given_values() {
  let buffer = InputBuffer::new(16, 64);
  assert_eq!(buffer.initial, 16);
  assert_eq!(buffer.max, 64);
}

#[test]
fn new_allows_zero_values() {
  let buffer = InputBuffer::new(0, 0);
  assert_eq!(buffer.initial, 0);
  assert_eq!(buffer.max, 0);
}

#[test]
fn new_allows_initial_equal_to_max() {
  let buffer = InputBuffer::new(32, 32);
  assert_eq!(buffer.initial, buffer.max);
}

#[test]
fn as_any_downcast_succeeds() {
  let boxed: Box<dyn Attribute> = Box::new(InputBuffer::new(8, 32));
  let downcast = boxed.as_any().downcast_ref::<InputBuffer>();
  assert!(downcast.is_some());
  let result = downcast.unwrap();
  assert_eq!(result.initial, 8);
  assert_eq!(result.max, 32);
}

#[test]
fn clone_box_produces_independent_copy() {
  let boxed: Box<dyn Attribute> = Box::new(InputBuffer::new(16, 64));
  let cloned = boxed.clone_box();
  let result = cloned.as_any().downcast_ref::<InputBuffer>().unwrap();
  assert_eq!(result.initial, 16);
  assert_eq!(result.max, 64);
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  let lhs = InputBuffer::new(16, 64);
  let rhs = InputBuffer::new(16, 64);
  assert!(lhs.eq_attr(&rhs as &dyn Any));
}

#[test]
fn eq_attr_returns_false_for_different_values() {
  let lhs = InputBuffer::new(16, 64);
  let rhs = InputBuffer::new(8, 32);
  assert!(!lhs.eq_attr(&rhs as &dyn Any));
}

#[test]
fn clone_preserves_values() {
  let original = InputBuffer::new(16, 64);
  let cloned = original.clone();
  assert_eq!(cloned, original);
}
