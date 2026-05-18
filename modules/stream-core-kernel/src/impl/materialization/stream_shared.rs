use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::{Stream, StreamState};
use crate::{StreamError, materialization::DriveOutcome, snapshot::StreamSnapshot};

/// Shared access point for a materialized [`Stream`].
pub(crate) struct StreamShared {
  inner: SharedLock<Stream>,
}

impl Clone for StreamShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl StreamShared {
  pub(crate) fn new(stream: Stream) -> Self {
    let inner = SharedLock::new_with_driver::<DefaultMutex<_>>(stream);
    Self { inner }
  }

  /// Returns the stream identifier.
  #[must_use]
  pub(crate) fn id(&self) -> u64 {
    self.with_read(Stream::id)
  }

  /// Returns the current stream state.
  #[must_use]
  pub(crate) fn state(&self) -> StreamState {
    self.with_read(Stream::state)
  }

  /// Cancels the stream execution.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when cancellation fails.
  pub(crate) fn cancel(&self) -> Result<(), StreamError> {
    self.with_write(Stream::cancel)
  }

  /// Requests graceful shutdown of the stream execution.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when shutdown cannot be requested.
  pub(crate) fn shutdown(&self) -> Result<(), StreamError> {
    self.with_write(Stream::shutdown)
  }

  /// Aborts the stream execution.
  pub(crate) fn abort(&self, error: &StreamError) {
    self.with_write(|stream| stream.abort(error));
  }

  /// Drives the stream once.
  #[must_use]
  pub(crate) fn drive(&self) -> DriveOutcome {
    self.with_write(Stream::drive)
  }

  /// Returns a diagnostic snapshot of this stream.
  #[must_use]
  pub(crate) fn snapshot(&self) -> StreamSnapshot {
    self.with_read(Stream::snapshot)
  }
}

impl SharedAccess<Stream> for StreamShared {
  fn with_read<R>(&self, f: impl FnOnce(&Stream) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Stream) -> R) -> R {
    self.inner.with_write(f)
  }
}
