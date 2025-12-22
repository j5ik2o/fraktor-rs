//! Tokio-friendly shared stream handle.

extern crate std;

use std::sync::{Arc, Mutex};

use crate::core::{StreamError, StreamHandleState, StreamState};

/// Shared handle for stream state inspection in std environments.
#[derive(Clone, Debug)]
pub struct StreamHandleShared {
  inner: Arc<Mutex<StreamHandleState>>,
}

impl StreamHandleShared {
  pub(crate) fn new(inner: Arc<Mutex<StreamHandleState>>) -> Self {
    Self { inner }
  }

  /// Returns the current stream state.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::ExecutorUnavailable` when the handle lock is poisoned.
  pub fn state(&self) -> Result<StreamState, StreamError> {
    let guard = self.inner.lock().map_err(|_| StreamError::ExecutorUnavailable)?;
    Ok(guard.state())
  }

  /// Requests stream cancellation.
  ///
  /// # Errors
  ///
  /// Returns `StreamError::ExecutorUnavailable` when the handle lock is poisoned.
  pub fn cancel(&self) -> Result<(), StreamError> {
    let mut guard = self.inner.lock().map_err(|_| StreamError::ExecutorUnavailable)?;
    guard.cancel()
  }
}
