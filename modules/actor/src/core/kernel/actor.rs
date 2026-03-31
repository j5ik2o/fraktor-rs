//! Actor primitives package.
//!
//! This module contains the core actor types and traits that form the foundation
//! of the actor system.

mod actor_cell;
mod actor_context;
mod actor_lifecycle;
pub mod actor_path;
pub mod actor_ref;
pub mod actor_selection;
mod actor_shared;
mod address;
mod child_ref;
mod context_pipe_task;
mod context_pipe_task_id;
mod context_pipe_waker;
pub mod dead_letter;
pub mod error;
pub mod extension;
pub mod lifecycle;
pub mod messaging;
mod pid;
mod pipe_spawn_error;
pub mod props;
/// Actor reference provider related types.
pub mod provider;
mod receive_state;
pub mod scheduler;
pub mod setup;
pub mod spawn;
pub mod supervision;

pub use actor_cell::ActorCell;
pub use actor_context::ActorContext;
pub(crate) use actor_context::STASH_OVERFLOW_REASON;
pub use actor_lifecycle::Actor;
pub(crate) use actor_shared::ActorShared;
pub use address::Address;
pub use child_ref::ChildRef;
pub use context_pipe_task_id::ContextPipeTaskId;
pub use pid::Pid;
pub use pipe_spawn_error::PipeSpawnError;
pub use receive_state::ReceiveState;
