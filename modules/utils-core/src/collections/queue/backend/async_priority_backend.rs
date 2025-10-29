use super::AsyncQueueBackend;

/// Extension trait for async backends supporting priority semantics.
pub trait AsyncPriorityBackend<T: Ord>: AsyncQueueBackend<T> {
  /// Returns a reference to the smallest element without removing it.
  fn peek_min(&self) -> Option<&T>;
}
