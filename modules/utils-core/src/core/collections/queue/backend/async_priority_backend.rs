use super::AsyncQueueBackend;
use crate::core::collections::{
  PriorityMessage, queue::backend::async_priority_backend_internal::AsyncPriorityBackendInternal,
};

/// Extension trait for async backends supporting priority semantics.
///
/// This trait is automatically sealed because it requires `AsyncPriorityBackendInternal` which is
/// `pub(crate)`. External crates cannot implement this trait.
#[allow(private_bounds)]
pub trait AsyncPriorityBackend<T: PriorityMessage>: AsyncPriorityBackendInternal<T> + AsyncQueueBackend<T> {}
