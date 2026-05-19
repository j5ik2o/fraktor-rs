//! Error package.
//!
//! This module contains error types.

mod actor_error;
mod actor_error_reason;
mod pipe_spawn_error;
mod send_error;
mod watch_conflict;
mod watch_registration_error;

pub use actor_error::ActorError;
pub use actor_error_reason::ActorErrorReason;
pub use pipe_spawn_error::PipeSpawnError;
pub use send_error::SendError;
pub use watch_conflict::WatchConflict;
pub use watch_registration_error::WatchRegistrationError;
