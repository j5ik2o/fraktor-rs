//! Context pipe primitives for bridging async futures into actor mailboxes.
//!
//! Corresponds to the `pipe_to_self` pattern in Pekko typed contexts.

mod context_pipe_waker_handle;
mod context_pipe_waker_handle_shared;
mod task;
mod task_id;
mod waker;

pub use context_pipe_waker_handle::ContextPipeWakerHandle;
pub use context_pipe_waker_handle_shared::ContextPipeWakerHandleShared;
pub(crate) use task::{ContextPipeFuture, ContextPipeTask};
pub use task_id::ContextPipeTaskId;
pub(crate) use waker::ContextPipeWaker;
