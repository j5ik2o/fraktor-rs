extern crate std;

/// Backend implementations for queues.
pub mod backend;

use crate::{
  core::collections::queue::{
    SyncQueue, SyncQueueShared,
    type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey},
  },
  std::sync_mutex::StdSyncMutex,
};

/// Type alias for an MPSC queue using [`std::sync::Mutex`].
pub type StdSyncMpscQueueShared<T, B, M = StdSyncMutex<SyncQueue<T, MpscKey, B>>> = SyncQueueShared<T, MpscKey, B, M>;

/// Type alias for an SPSC queue using [`std::sync::Mutex`].
pub type StdSyncSpscQueueShared<T, B, M = StdSyncMutex<SyncQueue<T, SpscKey, B>>> = SyncQueueShared<T, SpscKey, B, M>;

/// Type alias for a FIFO queue using [`std::sync::Mutex`].
pub type StdSyncFifoQueueShared<T, B, M = StdSyncMutex<SyncQueue<T, FifoKey, B>>> = SyncQueueShared<T, FifoKey, B, M>;

/// Type alias for a priority queue using [`std::sync::Mutex`].
pub type StdSyncPriorityQueueShared<T, B, M = StdSyncMutex<SyncQueue<T, PriorityKey, B>>> =
  SyncQueueShared<T, PriorityKey, B, M>;
