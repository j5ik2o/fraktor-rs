use super::StackError;
use crate::core::sync::SharedError;

#[test]
fn from_shared_error_poisoned() {
  let err = StackError::from(SharedError::Poisoned);
  assert_eq!(err, StackError::Disconnected);
}

#[test]
fn from_shared_error_borrow_conflict() {
  let err = StackError::from(SharedError::BorrowConflict);
  assert_eq!(err, StackError::WouldBlock);
}

#[test]
fn from_shared_error_interrupt_context() {
  let err = StackError::from(SharedError::InterruptContext);
  assert_eq!(err, StackError::WouldBlock);
}

#[test]
fn stack_error_is_debug() {
  let err = StackError::Empty;
  let debug_str = format!("{:?}", err);
  assert!(debug_str.contains("Empty"));
}

#[test]
fn stack_error_is_clone() {
  let err1 = StackError::Closed;
  let err2 = err1;
  assert_eq!(err1, err2);
}

#[test]
fn stack_error_is_copy() {
  let err1 = StackError::Full;
  let err2 = err1;
  assert_eq!(err1, err2);
}

#[test]
fn stack_error_partial_eq() {
  assert_eq!(StackError::Empty, StackError::Empty);
  assert_ne!(StackError::Empty, StackError::Closed);
}

#[test]
fn stack_error_eq() {
  let err1 = StackError::Disconnected;
  let err2 = StackError::Disconnected;
  assert_eq!(err1, err2);
}
