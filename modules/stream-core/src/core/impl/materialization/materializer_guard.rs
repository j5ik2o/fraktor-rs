use fraktor_utils_core_rs::core::sync::SharedAccess;

use super::{StreamHandleId, StreamState, materializer_session::StreamShared};
use crate::core::{
  SharedKillSwitch, StreamError, UniqueKillSwitch, materialization::DriveOutcome, snapshot::StreamSnapshot,
};

/// Handle owning the lifecycle of a stream execution.
pub struct StreamHandleImpl {
  id:     StreamHandleId,
  shared: StreamShared,
}

impl Clone for StreamHandleImpl {
  fn clone(&self) -> Self {
    Self { id: self.id, shared: self.shared.clone() }
  }
}

impl StreamHandleImpl {
  pub(crate) const fn new(id: StreamHandleId, shared: StreamShared) -> Self {
    Self { id, shared }
  }

  /// Returns the handle identifier.
  #[must_use]
  pub const fn id(&self) -> StreamHandleId {
    self.id
  }

  /// Returns the current stream state.
  #[must_use]
  pub fn state(&self) -> StreamState {
    self.shared.with_read(|stream| stream.state())
  }

  /// Cancels the stream execution.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when cancellation fails.
  pub fn cancel(&self) -> Result<(), StreamError> {
    self.shared.with_write(|stream| stream.cancel())
  }

  /// Drives the stream once.
  #[must_use]
  pub fn drive(&self) -> DriveOutcome {
    self.shared.with_write(|stream| stream.drive())
  }

  /// Returns a unique kill switch bound to this stream.
  #[must_use]
  pub fn unique_kill_switch(&self) -> UniqueKillSwitch {
    let state = self.shared.with_read(|stream| stream.kill_switch_state());
    UniqueKillSwitch::from_state(state)
  }

  /// Returns a shared kill switch bound to this stream.
  #[must_use]
  pub fn shared_kill_switch(&self) -> SharedKillSwitch {
    let state = self.shared.with_read(|stream| stream.kill_switch_state());
    SharedKillSwitch::from_state(state)
  }

  /// Returns a diagnostic snapshot of the stream behind this handle.
  #[must_use]
  pub fn snapshot(&self) -> StreamSnapshot {
    self.shared.with_read(|stream| stream.snapshot())
  }
}
