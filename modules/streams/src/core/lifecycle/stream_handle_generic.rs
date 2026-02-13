use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use super::{
  DriveOutcome, SharedKillSwitch, StreamError, StreamHandle, StreamHandleId, StreamState, UniqueKillSwitch,
  stream_shared::StreamSharedGeneric,
};

/// Handle owning the lifecycle of a stream execution.
pub struct StreamHandleGeneric<TB: RuntimeToolbox + 'static> {
  id:     StreamHandleId,
  shared: StreamSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for StreamHandleGeneric<TB> {
  fn clone(&self) -> Self {
    Self { id: self.id, shared: self.shared.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> StreamHandleGeneric<TB> {
  pub(crate) const fn new(id: StreamHandleId, shared: StreamSharedGeneric<TB>) -> Self {
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
}

impl<TB: RuntimeToolbox + 'static> StreamHandle for StreamHandleGeneric<TB> {
  fn id(&self) -> StreamHandleId {
    StreamHandleGeneric::id(self)
  }

  fn state(&self) -> StreamState {
    StreamHandleGeneric::state(self)
  }

  fn cancel(&self) -> Result<(), StreamError> {
    StreamHandleGeneric::cancel(self)
  }

  fn drive(&self) -> DriveOutcome {
    StreamHandleGeneric::drive(self)
  }
}
