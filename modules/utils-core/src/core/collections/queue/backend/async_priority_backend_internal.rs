use super::AsyncQueueBackend;
use crate::core::collections::PriorityMessage;

/// Extension trait for async backends supporting priority semantics.
pub(crate) trait AsyncPriorityBackendInternal<T: PriorityMessage>: AsyncQueueBackend<T> {
  /// Returns a reference to the smallest element without removing it.
  fn peek_min(&self) -> Option<&T>;
}
