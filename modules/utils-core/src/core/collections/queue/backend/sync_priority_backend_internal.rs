use super::SyncQueueBackend;
use crate::core::collections::PriorityMessage;

/// Extension trait for backends supporting priority semantics.
pub(crate) trait SyncPriorityBackendInternal<T: PriorityMessage>: SyncQueueBackend<T> {
  /// Returns a reference to the smallest element without removing it.
  fn peek_min(&self) -> Option<&T>;
}
