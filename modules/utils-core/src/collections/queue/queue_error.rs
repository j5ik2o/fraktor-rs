use crate::sync::SharedError;

/// Errors that occur during queue operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError<T> {
  /// The queue is full and cannot accept more elements. Contains the element that was attempted to
  /// be added.
  Full(T),
  /// The offer operation to the queue failed. Contains the element that was attempted to be
  /// provided.
  OfferError(T),
  /// The queue is closed. Contains the element that was attempted to be sent.
  Closed(T),
  /// The connection to the queue has been disconnected.
  Disconnected,
  /// The queue has no elements to consume.
  Empty,
  /// The operation would block and cannot proceed in the current context.
  WouldBlock,
  /// An allocation error occurred while attempting to grow or manage storage. Contains the element
  /// associated with the failed allocation.
  AllocError(T),
}

impl<T> QueueError<T> {
  /// Extracts the payload carried by variants that preserve the element on failure.
  #[must_use]
  pub fn into_item(self) -> Option<T> {
    match self {
      | Self::Full(item) | Self::OfferError(item) | Self::Closed(item) | Self::AllocError(item) => Some(item),
      | Self::Disconnected | Self::Empty | Self::WouldBlock => None,
    }
  }
}

impl<T> From<SharedError> for QueueError<T> {
  fn from(err: SharedError) -> Self {
    match err {
      | SharedError::Poisoned => QueueError::Disconnected,
      | SharedError::BorrowConflict | SharedError::InterruptContext => QueueError::WouldBlock,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

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
}
