use super::SyncQueueBackend;
use crate::core::collections::{
  PriorityMessage, queue::backend::sync_priority_backend_internal::SyncPriorityBackendInternal,
};

mod priority_entry;
#[cfg(test)]
mod tests;

pub(crate) use priority_entry::PriorityEntry;

/// Extension trait for backends supporting priority semantics.
///
/// This trait is automatically sealed because it requires `SyncPriorityBackendInternal` which is
/// `pub(crate)`. External crates cannot implement this trait.
#[allow(private_bounds)]
pub trait SyncPriorityBackend<T: PriorityMessage>: SyncPriorityBackendInternal<T> + SyncQueueBackend<T> {}
