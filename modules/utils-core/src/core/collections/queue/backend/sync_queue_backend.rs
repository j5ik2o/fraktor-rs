use crate::core::collections::queue::backend::SyncQueueBackendInternal;

/// Backend trait responsible for queue operations on top of a storage implementation.
///
/// This trait is automatically sealed because it requires `SyncQueueBackendInternal` which is
/// `pub(crate)`. External crates cannot implement this trait.
#[allow(private_bounds)]
pub trait SyncQueueBackend<T>: SyncQueueBackendInternal<T> {}
