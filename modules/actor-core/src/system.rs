//! System package.
//!
//! This module contains the actor system management.

mod base;
mod root_guardian_actor;
mod register_extra_top_level_error;
mod system_guardian_actor;
mod system_guardian_protocol;
mod system_state;
pub use base::{ActorSystem, ActorSystemGeneric};
pub use register_extra_top_level_error::RegisterExtraTopLevelError;
pub(crate) use root_guardian_actor::RootGuardianActor;
pub(crate) use system_guardian_actor::SystemGuardianActor;
pub use system_guardian_protocol::SystemGuardianProtocol;
pub use system_state::{FailureOutcome, GuardianKind, SystemState, SystemStateGeneric};
