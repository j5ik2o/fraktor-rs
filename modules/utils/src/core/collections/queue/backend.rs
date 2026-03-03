//! Backend layer traits and supporting enums for queue operations.

mod binary_heap_priority_backend;
mod priority_backend_config;
/// Priority backend implementations for synchronous queues.
pub mod sync_priority_backend;
mod sync_priority_backend_internal;
mod sync_queue_backend;
mod sync_queue_backend_internal;
mod vec_deque_backend;

pub use binary_heap_priority_backend::BinaryHeapPriorityBackend;
pub use priority_backend_config::PriorityBackendConfig;
pub(crate) use sync_priority_backend_internal::SyncPriorityBackendInternal;
pub use sync_queue_backend::SyncQueueBackend;
pub(crate) use sync_queue_backend_internal::SyncQueueBackendInternal;
pub use vec_deque_backend::VecDequeBackend;
