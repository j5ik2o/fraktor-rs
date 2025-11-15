//! Storage layer abstractions for queue backends.

mod queue_storage;
mod vec_deque_storage;

pub use queue_storage::QueueStorage;
pub use vec_deque_storage::VecDequeStorage;
