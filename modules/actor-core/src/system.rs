//! System package.
//!
//! This module contains the actor system management.

mod base;
mod system_state;
pub use base::{ActorSystem, ActorSystemGeneric};
pub use system_state::{FailureOutcome, SystemState, SystemStateGeneric};
