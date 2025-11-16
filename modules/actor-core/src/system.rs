//! System package.
//!
//! This module contains the actor system management.

mod actor_path_handle;
mod actor_path_registry;
mod actor_system_build_error;
mod actor_system_builder;
mod authority_state;
mod base;
mod guardian_kind;
mod register_extra_top_level_error;
mod remote_authority;
mod remote_authority_error;
mod reservation_policy;
mod root_guardian_actor;
mod system_guardian_actor;
mod system_guardian_protocol;
mod system_state;

pub use actor_path_handle::ActorPathHandle;
pub use actor_path_registry::ActorPathRegistry;
pub use actor_system_build_error::ActorSystemBuildError;
pub use actor_system_builder::ActorSystemBuilder;
pub use authority_state::AuthorityState;
pub use base::{ActorSystem, ActorSystemGeneric};
pub use guardian_kind::GuardianKind;
pub use register_extra_top_level_error::RegisterExtraTopLevelError;
pub use remote_authority::{RemoteAuthorityManager, RemoteAuthorityManagerGeneric};
pub use remote_authority_error::RemoteAuthorityError;
pub use reservation_policy::ReservationPolicy;
pub(crate) use root_guardian_actor::RootGuardianActor;
pub(crate) use system_guardian_actor::SystemGuardianActor;
pub use system_guardian_protocol::SystemGuardianProtocol;
pub use system_state::{FailureOutcome, SystemState, SystemStateGeneric};
