use super::QueueError;
use crate::core::sync::SharedError;

#[test]
fn queue_error_full_variant() {
  let error = QueueError::Full(42);
  assert_eq!(error.into_item(), Some(42));
}

#[test]
fn queue_error_offer_error_variant() {
  let error = QueueError::OfferError(100);
  assert_eq!(error.into_item(), Some(100));
}

#[test]
fn queue_error_closed_variant() {
  let error = QueueError::Closed("test");
  assert_eq!(error.into_item(), Some("test"));
}

#[test]
fn queue_error_alloc_error_variant() {
  let error = QueueError::AllocError(777);
  assert_eq!(error.into_item(), Some(777));
}

#[test]
fn queue_error_disconnected_variant() {
  let error: QueueError<i32> = QueueError::Disconnected;
  assert_eq!(error.into_item(), None);
}

#[test]
fn queue_error_empty_variant() {
  let error: QueueError<String> = QueueError::Empty;
  assert_eq!(error.into_item(), None);
}

#[test]
fn queue_error_would_block_variant() {
  let error: QueueError<()> = QueueError::WouldBlock;
  assert_eq!(error.into_item(), None);
}

#[test]
fn queue_error_from_shared_error_poisoned() {
  let shared_error = SharedError::Poisoned;
  let queue_error: QueueError<i32> = shared_error.into();
  assert_eq!(queue_error, QueueError::Disconnected);
}

#[test]
fn queue_error_from_shared_error_borrow_conflict() {
  let shared_error = SharedError::BorrowConflict;
  let queue_error: QueueError<String> = shared_error.into();
  assert_eq!(queue_error, QueueError::WouldBlock);
}

#[test]
fn queue_error_from_shared_error_interrupt_context() {
  let shared_error = SharedError::InterruptContext;
  let queue_error: QueueError<()> = shared_error.into();
  assert_eq!(queue_error, QueueError::WouldBlock);
}

#[test]
fn queue_error_clone_works() {
  let original = QueueError::Full(5);
  let cloned = original.clone();
  assert_eq!(cloned.into_item(), Some(5));
}

#[test]
fn queue_error_debug_format() {
  let error = QueueError::Full(10);
  let debug_str = format!("{:?}", error);
  assert!(debug_str.contains("Full"));
}

#[test]
fn queue_error_partial_eq() {
  assert_eq!(QueueError::Full(1), QueueError::Full(1));
  assert_ne!(QueueError::Full(1), QueueError::Full(2));
  assert_eq!(QueueError::<i32>::Empty, QueueError::<i32>::Empty);
  assert_ne!(QueueError::<i32>::Empty, QueueError::<i32>::Disconnected);
}
