//! no_std-friendly queue primitives shared between runtimes.

mod queue_size;
/// Queue trait definitions shared across all backends.
pub mod traits;

/// Multi-producer/single-consumer queue abstractions.
pub mod mpsc;
/// Priority-ordered queue abstractions.
pub mod priority;
/// Ring-buffer-based queue implementations and utilities.
pub mod ring;

pub use mpsc::{MpscBackend, MpscBuffer, MpscHandle, MpscQueue, RingBufferBackend};
pub use priority::{PriorityMessage, PriorityQueue, DEFAULT_PRIORITY, PRIORITY_LEVELS};
pub use crate::collections::queue::QueueError;
pub use queue_size::QueueSize;
pub use ring::{
    RingBackend, RingBuffer, RingBufferStorage, RingHandle, RingQueue, RingStorageBackend, DEFAULT_CAPACITY,
};
pub use traits::{
    QueueBase, QueueHandle as QueueRwHandle, QueueHandle, QueueReader, QueueRw, QueueStorage, QueueWriter,
};
