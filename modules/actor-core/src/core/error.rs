//! Error package.
//!
//! This module contains error types.

mod actor_error;
mod actor_error_reason;
mod send_error;

pub use actor_error::ActorError;
pub use actor_error_reason::ActorErrorReason;
pub use send_error::SendError;
