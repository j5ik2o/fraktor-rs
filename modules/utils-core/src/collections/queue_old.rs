//! no_std-friendly queue primitives shared between runtimes.

#![allow(clippy::no_parent_reexport)]

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
pub use priority::{DEFAULT_PRIORITY, PRIORITY_LEVELS, PriorityQueue};
pub use queue_size::QueueSize;
pub use ring::{
  DEFAULT_CAPACITY, RingBackend, RingBuffer, RingBufferStorage, RingHandle, RingQueue, RingStorageBackend,
};
pub use traits::{
  QueueBase, QueueHandle as QueueRwHandle, QueueHandle, QueueReader, QueueRw, QueueStorage, QueueWriter,
};
