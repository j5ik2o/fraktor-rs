extern crate std;

/// Backend implementations for queues.
pub mod backend;

use crate::{
  core::collections::queue::{SyncQueue, SyncQueueShared, type_keys::MpscKey},
  std::sync_mutex::StdSyncMutex,
};

/// Type alias for an MPSC queue using [`std::sync::Mutex`].
#[allow(dead_code)]
pub(crate) type StdSyncMpscQueueShared<T, B, M = StdSyncMutex<SyncQueue<T, MpscKey, B>>> =
  SyncQueueShared<T, MpscKey, B, M>;
