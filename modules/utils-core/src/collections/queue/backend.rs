//! Backend layer traits and supporting enums for queue operations.

mod async_priority_backend;
mod async_queue_backend;
mod sync_queue_async_adapter;
mod sync_priority_backend;
mod sync_queue_backend;
mod vec_ring_backend;

pub use async_priority_backend::AsyncPriorityBackend;
pub use async_queue_backend::AsyncQueueBackend;
pub use crate::collections::queue::offer_outcome::OfferOutcome;
pub use crate::collections::queue::overflow_policy::OverflowPolicy;
pub use sync_queue_async_adapter::SyncQueueAsyncAdapter;
pub use sync_priority_backend::SyncPriorityBackend;
pub use sync_queue_backend::SyncQueueBackend;
pub use vec_ring_backend::VecRingBackend;
