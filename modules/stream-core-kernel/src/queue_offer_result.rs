use super::StreamError;

#[cfg(test)]
#[path = "queue_offer_result_test.rs"]
mod tests;

/// Result of offering an element into a source queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueOfferResult {
  /// The element was enqueued.
  Enqueued,
  /// The element was dropped due to overflow strategy.
  Dropped,
  /// The queue is already closed.
  QueueClosed,
  /// The offer failed with an error.
  Failure(StreamError),
}
