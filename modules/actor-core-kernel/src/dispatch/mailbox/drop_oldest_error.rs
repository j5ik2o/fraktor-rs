//! Error returned by [`QueueStateHandle::drop_oldest_and_offer`] when the offer step fails.

use fraktor_utils_core_rs::core::collections::queue::QueueError;

/// Error raised when `drop_oldest_and_offer` fails during the final `offer` step.
///
/// Surfaces the evicted element (if any) alongside the queue error so the caller
/// (mailbox layer) can still forward the evicted element to dead letters instead
/// of losing it silently.
#[derive(Debug)]
pub(crate) struct DropOldestError<T> {
  /// Underlying queue error returned by the backend.
  pub(crate) error:   QueueError<T>,
  /// Element evicted before the failing `offer`, if capacity forced an eviction.
  pub(crate) evicted: Option<T>,
}
