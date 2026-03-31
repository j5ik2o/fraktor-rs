//! Context pipe primitives for bridging async futures into actor mailboxes.
//!
//! Corresponds to the `pipe_to_self` pattern in Pekko typed contexts.

mod task;
mod task_id;
mod waker;

pub(crate) use task::{ContextPipeFuture, ContextPipeTask};
pub(crate) use waker::ContextPipeWaker;
pub use task_id::ContextPipeTaskId;
