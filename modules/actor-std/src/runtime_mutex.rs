//! Runtime mutex alias for std environments.

use cellactor_utils_core_rs::sync::sync_mutex_like::StdSyncMutex;

/// Mutex type exposed by actor-std, backed by [`std::sync::Mutex`].
pub type ActorRuntimeMutex<T> = StdSyncMutex<T>;
