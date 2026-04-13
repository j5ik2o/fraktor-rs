//! Shared wrapper for context-pipe waker handles.

use spin::Once;

use crate::core::kernel::actor::{
  context_pipe::context_pipe_waker_handle::ContextPipeWakerHandle, messaging::system_message::SystemMessage,
};

/// Shared wrapper that provides lock-free access to a [`ContextPipeWakerHandle`].
///
/// The handle is set once at construction and thereafter only read via
/// `spin::Once::get()` (a single atomic load).
pub struct ContextPipeWakerHandleShared {
  inner: Once<ContextPipeWakerHandle>,
}

impl ContextPipeWakerHandleShared {
  /// Creates a new shared wrapper, immediately initializing the inner handle.
  #[must_use]
  pub fn new(handle: ContextPipeWakerHandle) -> Self {
    Self { inner: Once::initialized(handle) }
  }

  pub(crate) fn wake(&self) {
    // spin::Once::get() は atomic load のみ — ロック不要
    let handle = self.inner.get().expect("ContextPipeWakerHandle not initialized");
    let (system, pid, task) = (handle.system.clone(), handle.pid, handle.task);
    if let Err(error) = system.send_system_message(pid, SystemMessage::PipeTask(task)) {
      system.record_send_error(Some(pid), &error);
    }
  }
}
