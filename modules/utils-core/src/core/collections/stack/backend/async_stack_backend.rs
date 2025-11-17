use async_trait::async_trait;

use super::AsyncStackBackendInternal;

/// Async-compatible backend trait for stack operations.
///
/// This trait is automatically sealed because it requires `AsyncStackBackendInternal` which is
/// `pub(crate)`. External crates cannot implement this trait.
#[async_trait(?Send)]
#[allow(private_bounds)]
pub trait AsyncStackBackend<T>: AsyncStackBackendInternal<T> {}
