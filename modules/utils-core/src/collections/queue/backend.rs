//! Backend layer traits and supporting enums for queue operations.

mod binary_heap_priority_backend;
mod priority_backend_config;
mod priority_entry;
mod sync_queue_backend;
mod sync_queue_backend_internal;
mod vec_deque_backend;

pub use binary_heap_priority_backend::BinaryHeapPriorityBackend;
pub use priority_backend_config::PriorityBackendConfig;
pub use sync_queue_backend::SyncQueueBackend;
pub(crate) use sync_queue_backend_internal::SyncQueueBackendInternal;
pub use vec_deque_backend::VecDequeBackend;
