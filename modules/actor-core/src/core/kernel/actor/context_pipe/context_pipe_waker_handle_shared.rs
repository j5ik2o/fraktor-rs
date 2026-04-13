//! Shared wrapper for context-pipe waker handles.

use fraktor_utils_core_rs::core::sync::{SharedLock, DefaultMutex};

use crate::core::kernel::actor::{
  context_pipe::context_pipe_waker_handle::ContextPipeWakerHandle, messaging::system_message::SystemMessage,
};

/// Shared wrapper that serializes access to a [`ContextPipeWakerHandle`].
pub struct ContextPipeWakerHandleShared {
  inner: SharedLock<ContextPipeWakerHandle>,
}

impl ContextPipeWakerHandleShared {
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub fn new(handle: ContextPipeWakerHandle) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<DefaultMutex<_>>(handle))
  }

  /// Creates a shared wrapper from an already materialized shared lock.
  #[must_use]
  pub const fn from_shared_lock(lock: SharedLock<ContextPipeWakerHandle>) -> Self {
    Self { inner: lock }
  }

  pub(crate) fn wake(&self) {
    // ロック保持中に send_system_message を呼ぶとデッドロックするため、
    // ロックスコープ内でクローンを取得し、解放後に送信する
    let (system, pid, task) = self.inner.with_lock(|guard| (guard.system.clone(), guard.pid, guard.task));
    if let Err(error) = system.send_system_message(pid, SystemMessage::PipeTask(task)) {
      system.record_send_error(Some(pid), &error);
    }
  }
}

impl Clone for ContextPipeWakerHandleShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
