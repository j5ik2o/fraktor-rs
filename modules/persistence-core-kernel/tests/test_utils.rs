//! Shared test utilities for persistence integration tests.

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

/// Creates a shared lock wrapping the given value.
pub fn shared_mutex<T: Send + 'static>(value: T) -> ArcShared<SpinSyncMutex<T>> {
  ArcShared::new(SpinSyncMutex::new(value))
}
