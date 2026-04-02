//! Actor primitives package.
//!
//! This module contains the core actor types and traits that form the foundation
//! of the actor system.

mod actor_cell;
mod actor_context;
mod actor_lifecycle;
pub mod actor_path;
pub mod actor_ref;
/// Actor reference provider related types.
pub mod actor_ref_provider;
pub mod actor_selection;
mod actor_shared;
mod address;
mod child_ref;
mod classic_timer_scheduler;
pub mod context_pipe;
pub mod error;
pub mod extension;
pub mod lifecycle;
pub mod messaging;
mod pid;
pub mod props;
mod receive_state;
pub mod scheduler;
pub mod setup;
pub mod spawn;
mod stash_overflow_error;
pub mod supervision;

pub use actor_cell::ActorCell;
pub use actor_context::ActorContext;
pub(crate) use actor_context::STASH_OVERFLOW_REASON;
pub use actor_lifecycle::Actor;
pub(crate) use actor_shared::ActorShared;
pub use address::Address;
pub use child_ref::ChildRef;
pub use classic_timer_scheduler::ClassicTimerScheduler;
pub use pid::Pid;
pub use receive_state::ReceiveState;
pub use stash_overflow_error::StashOverflowError;
