//! Queue abstractions rebuilt for the v2 collections layer.

pub mod backend;
pub mod capabilities;
mod sync_queue_shared;

pub use sync_queue_shared::SyncQueueShared;

mod offer_outcome;
mod overflow_policy;
mod queue_error;
mod sync_queue;
#[cfg(test)]
#[path = "queue_test.rs"]
mod tests;
pub use offer_outcome::OfferOutcome;
pub use overflow_policy::OverflowPolicy;
pub use queue_error::QueueError;
pub use sync_queue::*;
