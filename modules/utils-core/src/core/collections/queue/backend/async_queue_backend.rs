use async_trait::async_trait;

use super::AsyncQueueBackendInternal;

/// Async-compatible backend trait for queue operations.
#[async_trait(?Send)]
#[allow(private_bounds)]
pub trait AsyncQueueBackend<T>: AsyncQueueBackendInternal<T> {}
