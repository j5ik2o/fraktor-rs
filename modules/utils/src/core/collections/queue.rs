//! Queue abstractions rebuilt for the v2 collections layer.

pub mod backend;
pub mod capabilities;
mod sync_mpsc_consumer_shared;
mod sync_mpsc_producer_shared;
mod sync_queue_shared;
mod sync_spsc_consumer_shared;
mod sync_spsc_producer_shared;
pub mod type_keys;

pub use sync_mpsc_consumer_shared::SyncMpscConsumerShared;
pub use sync_mpsc_producer_shared::SyncMpscProducerShared;
pub use sync_queue_shared::{
  SyncFifoQueueShared, SyncMpscQueueShared, SyncPriorityQueueShared, SyncQueueShared, SyncSpscQueueShared,
};
pub use sync_spsc_consumer_shared::SyncSpscConsumerShared;
pub use sync_spsc_producer_shared::SyncSpscProducerShared;

mod offer_outcome;
mod overflow_policy;
mod queue_error;
mod sync_queue;
#[cfg(test)]
mod tests;
pub use offer_outcome::OfferOutcome;
pub use overflow_policy::OverflowPolicy;
pub use queue_error::QueueError;
pub use sync_queue::*;
