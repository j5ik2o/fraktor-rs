//! System package.
//!
//! This module contains the actor system management.

mod root;
mod system_state;
pub use root::{ActorSystem, ActorSystemGeneric};
pub use system_state::SystemState;
