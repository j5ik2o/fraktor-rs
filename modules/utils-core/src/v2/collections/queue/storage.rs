//! Storage layer abstractions for queue backends.

mod queue_storage;
mod vec_ring_storage;

pub use queue_storage::QueueStorage;
pub use vec_ring_storage::VecRingStorage;
