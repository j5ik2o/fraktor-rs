//! Factory contract for shared context-pipe waker handles.

use crate::core::kernel::actor::context_pipe::{
  ContextPipeWakerHandle, context_pipe_waker_handle_shared::ContextPipeWakerHandleShared,
};

/// Materializes [`ContextPipeWakerHandleShared`] instances.
pub trait ContextPipeWakerHandleSharedFactory: Send + Sync {
  /// Wraps a context-pipe waker handle into a shared handle.
  fn create_context_pipe_waker_handle_shared(&self, handle: ContextPipeWakerHandle) -> ContextPipeWakerHandleShared;
}
