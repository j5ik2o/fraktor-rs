//! Queue abstractions rebuilt for the v2 collections layer.

mod async_mpsc_consumer_shared;
mod async_mpsc_producer_shared;
mod async_queue_shared;
mod async_spsc_consumer_shared;
mod async_spsc_producer_shared;
pub mod backend;
pub mod capabilities;
mod deque_backend;
mod sync_mpsc_consumer_shared;
mod sync_mpsc_producer_shared;
mod sync_queue_shared;
mod sync_spsc_consumer_shared;
mod sync_spsc_producer_shared;
pub mod type_keys;

pub use async_mpsc_consumer_shared::AsyncMpscConsumerShared;
pub use async_mpsc_producer_shared::AsyncMpscProducerShared;
pub use async_queue_shared::{AsyncFifoQueue, AsyncMpscQueue, AsyncPriorityQueue, AsyncQueueShared, AsyncSpscQueue};
pub use async_spsc_consumer_shared::AsyncSpscConsumerShared;
pub use async_spsc_producer_shared::AsyncSpscProducerShared;
pub use backend::{
  AsyncPriorityBackend, AsyncQueueBackend, OfferOutcome, OverflowPolicy, SyncQueueAsyncAdapter, SyncQueueBackend,
  VecDequeBackend, sync_priority_backend::SyncPriorityBackend,
};
pub use capabilities::{
  MultiProducer, QueueCapability, QueueCapabilityError, QueueCapabilityRegistry, QueueCapabilitySet, SingleConsumer,
  SingleProducer, SupportsPeek,
};
pub use deque_backend::{DequeBackend, DequeBackendGeneric, DequeOfferFuture};
pub use sync_mpsc_consumer_shared::SyncMpscConsumerShared;
pub use sync_mpsc_producer_shared::SyncMpscProducerShared;
pub use sync_queue_shared::{FifoQueue, MpscQueue, PriorityQueue, SpscQueue, SyncQueueShared};
pub use sync_spsc_consumer_shared::SyncSpscConsumerShared;
pub use sync_spsc_producer_shared::SyncSpscProducerShared;
pub use type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey, TypeKey};

mod offer_outcome;
mod overflow_policy;
mod queue_error;
#[cfg(test)]
mod tests;
pub use queue_error::QueueError;

/// Default shared queue alias backed by [`VecDequeBackend`].
pub type SyncFifoQueueShared<T, K = FifoKey> = SyncQueueShared<T, K, VecDequeBackend<T>>;

/// Default async shared queue alias backed by [`VecDequeBackend`] via the sync adapter.
pub type AsyncFifoQueueShared<T, K = FifoKey> = AsyncQueueShared<T, K, SyncQueueAsyncAdapter<T, VecDequeBackend<T>>>;
