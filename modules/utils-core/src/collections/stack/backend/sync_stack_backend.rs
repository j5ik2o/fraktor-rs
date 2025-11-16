use crate::collections::stack::backend::SyncStackBackendInternal;

/// Backend trait responsible for stack operations on top of a storage implementation.
///
/// This trait is automatically sealed because it requires `SyncStackBackendInternal` which is
/// `pub(crate)`. External crates cannot implement this trait.
#[allow(private_bounds)]
pub trait SyncStackBackend<T>: SyncStackBackendInternal<T> {}
