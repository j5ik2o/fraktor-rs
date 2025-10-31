pub use self::{
  binary_heap_priority_backend::BinaryHeapPriorityBackend, priority_backend_config::PriorityBackendConfig,
};
use super::SyncQueueBackend;
use crate::collections::PriorityMessage;

mod binary_heap_priority_backend;
mod priority_backend_config;
mod priority_entry;
#[cfg(test)]
mod tests;

/// Extension trait for backends supporting priority semantics.
pub trait SyncPriorityBackend<T: PriorityMessage>: SyncQueueBackend<T> {
  /// Returns a reference to the smallest element without removing it.
  fn peek_min(&self) -> Option<&T>;
}
