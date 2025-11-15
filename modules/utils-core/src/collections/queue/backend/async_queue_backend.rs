use alloc::boxed::Box;

use async_trait::async_trait;

use super::{AsyncQueueBackendInternal, OfferOutcome};
use crate::collections::{queue::QueueError, wait::WaitShared};

/// Async-compatible backend trait for queue operations.
#[async_trait(?Send)]
pub trait AsyncQueueBackend<T>: AsyncQueueBackendInternal<T> {
}
