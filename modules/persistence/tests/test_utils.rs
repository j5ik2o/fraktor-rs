//! Shared test utilities for persistence integration tests.

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

/// Creates a shared mutex wrapping the given value.
pub fn shared_mutex<T: Send + 'static>(value: T) -> ArcShared<RuntimeMutex<T>> {
  ArcShared::new(RuntimeMutex::new(value))
}
