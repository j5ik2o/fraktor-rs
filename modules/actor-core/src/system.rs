//! System package.
//!
//! This module contains the actor system management.

pub mod dispatcher;
mod root;
mod system_state;
pub use root::{ActorSystem, ActorSystemGeneric};
pub use system_state::SystemState;
