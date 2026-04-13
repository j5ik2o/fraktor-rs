//! Shared wrapper for context-pipe waker handles.

use crate::core::kernel::actor::{
  context_pipe::context_pipe_waker_handle::ContextPipeWakerHandle, messaging::system_message::SystemMessage,
};

/// Shared wrapper that holds a [`ContextPipeWakerHandle`] directly.
///
/// The handle is immutable after construction — no lock or atomic
/// indirection is needed on the read path.
pub struct ContextPipeWakerHandleShared {
  inner: ContextPipeWakerHandle,
}

impl ContextPipeWakerHandleShared {
  /// Creates a new shared wrapper storing the handle directly.
  #[must_use]
  pub fn new(handle: ContextPipeWakerHandle) -> Self {
    Self { inner: handle }
  }

  pub(crate) fn wake(&self) {
    let (system, pid, task) = (self.inner.system.clone(), self.inner.pid, self.inner.task);
    if let Err(error) = system.send_system_message(pid, SystemMessage::PipeTask(task)) {
      system.record_send_error(Some(pid), &error);
    }
  }
}
