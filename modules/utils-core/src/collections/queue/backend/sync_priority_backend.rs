pub use self::priority_backend_config::PriorityBackendConfig;
pub use super::binary_heap_priority_backend::BinaryHeapPriorityBackend;
use super::SyncQueueBackend;
use crate::collections::{
  PriorityMessage, queue::backend::sync_priority_backend_internal::SyncPriorityBackendInternal,
};

pub(in crate::collections::queue::backend) mod priority_backend_config;
pub(in crate::collections::queue::backend) mod priority_entry;
#[cfg(test)]
mod tests;

/// Extension trait for backends supporting priority semantics.
///
/// This trait is automatically sealed because it requires `SyncPriorityBackendInternal` which is
/// `pub(crate)`. External crates cannot implement this trait.
#[allow(private_bounds)]
pub trait SyncPriorityBackend<T: PriorityMessage>: SyncPriorityBackendInternal<T> + SyncQueueBackend<T> {}
