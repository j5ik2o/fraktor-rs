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
