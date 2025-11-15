//! Queue abstractions rebuilt for the v2 collections layer.

mod async_mpsc_consumer;
mod async_mpsc_producer;
mod async_queue;
mod async_spsc_consumer;
mod async_spsc_producer;
pub mod backend;
pub mod capabilities;
mod deque_backend;
mod sync_mpsc_consumer;
mod sync_mpsc_producer;
mod sync_queue;
mod sync_spsc_consumer;
mod sync_spsc_producer;
pub mod type_keys;

pub use async_mpsc_consumer::AsyncMpscConsumer;
pub use async_mpsc_producer::AsyncMpscProducer;
pub use async_queue::{AsyncFifoQueue, AsyncMpscQueue, AsyncPriorityQueue, AsyncQueue, AsyncSpscQueue};
pub use async_spsc_consumer::AsyncSpscConsumer;
pub use async_spsc_producer::AsyncSpscProducer;
pub use backend::{
  AsyncPriorityBackend, AsyncQueueBackend, OfferOutcome, OverflowPolicy, SyncQueueAsyncAdapter, SyncQueueBackend,
  VecDequeBackend, sync_priority_backend::SyncPriorityBackend,
};
pub use capabilities::{
  MultiProducer, QueueCapability, QueueCapabilityError, QueueCapabilityRegistry, QueueCapabilitySet, SingleConsumer,
  SingleProducer, SupportsPeek,
};
pub use deque_backend::{DequeBackend, DequeBackendGeneric, DequeOfferFuture};
pub use sync_mpsc_consumer::SyncMpscConsumer;
pub use sync_mpsc_producer::SyncMpscProducer;
pub use sync_queue::{FifoQueue, MpscQueue, PriorityQueue, SpscQueue, SyncQueue};
pub use sync_spsc_consumer::SyncSpscConsumer;
pub use sync_spsc_producer::SyncSpscProducer;
pub use type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey, TypeKey};

mod offer_outcome;
mod overflow_policy;
mod queue_error;
#[cfg(test)]
mod tests;
pub use queue_error::QueueError;

/// Default shared queue alias backed by [`VecDequeBackend`].
pub type SharedVecDequeQueue<T, K = FifoKey> = SyncQueue<T, K, VecDequeBackend<T>>;

/// Default async shared queue alias backed by [`VecDequeBackend`] via the sync adapter.
pub type AsyncSharedVecDequeQueue<T, K = FifoKey> = AsyncQueue<T, K, SyncQueueAsyncAdapter<T, VecDequeBackend<T>>>;
