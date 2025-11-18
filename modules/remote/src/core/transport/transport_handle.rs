//! Handle returned by transport listeners.

use alloc::{string::String, vec::Vec};
use core::{
  mem,
  sync::atomic::{AtomicU64, Ordering},
};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

struct TransportHandleInner {
  authority: String,
  frames:    ToolboxMutex<Vec<Vec<u8>>, NoStdToolbox>,
  sequence:  AtomicU64,
}

impl TransportHandleInner {
  fn new(authority: impl Into<String>) -> Self {
    Self {
      authority: authority.into(),
      frames:    <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      sequence:  AtomicU64::new(0),
    }
  }
}

/// Public handle used by integration tests to inspect inbound frames.
#[derive(Clone)]
pub struct TransportHandle {
  inner: ArcShared<TransportHandleInner>,
}

impl TransportHandle {
  pub(crate) fn new(authority: impl Into<String>) -> Self {
    Self { inner: ArcShared::new(TransportHandleInner::new(authority)) }
  }

  /// Records a frame (used by transports).
  pub(crate) fn push_frame(&self, payload: Vec<u8>) {
    self.inner.frames.lock().push(payload);
  }

  /// Returns and clears recorded frames.
  #[must_use]
  pub fn take_frames(&self) -> Vec<Vec<u8>> {
    mem::take(&mut *self.inner.frames.lock())
  }

  /// Returns the current buffered frame count.
  #[must_use]
  pub fn buffered(&self) -> usize {
    self.inner.frames.lock().len()
  }

  /// Returns the authority represented by this handle.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.inner.authority
  }

  /// Generates a monotonic identifier.
  #[must_use]
  pub fn next_sequence(&self) -> u64 {
    self.inner.sequence.fetch_add(1, Ordering::Relaxed)
  }
}
