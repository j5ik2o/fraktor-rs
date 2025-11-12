//! System package.
//!
//! This module contains the actor system management.

pub mod actor_path_registry;
mod base;
mod guardian_kind;
mod register_extra_top_level_error;
pub mod remote_authority;
mod root_guardian_actor;
mod system_guardian_actor;
mod system_guardian_protocol;
mod system_state;
pub use actor_path_registry::{ActorPathHandle, ActorPathRegistry};
pub use base::{ActorSystem, ActorSystemGeneric};
pub use guardian_kind::GuardianKind;
pub use register_extra_top_level_error::RegisterExtraTopLevelError;
pub use remote_authority::{AuthorityState, RemoteAuthorityManager, RemoteAuthorityManagerGeneric};
pub(crate) use root_guardian_actor::RootGuardianActor;
pub(crate) use system_guardian_actor::SystemGuardianActor;
pub use system_guardian_protocol::SystemGuardianProtocol;
pub use system_state::{FailureOutcome, SystemState, SystemStateGeneric};
