//! Mutex alias used by actor-core.

use cellactor_utils_core_rs::sync::sync_mutex_like::SpinSyncMutex;

/// Mutex type employed across actor-core.
pub type ActorRuntimeMutex<T> = SpinSyncMutex<T>;
