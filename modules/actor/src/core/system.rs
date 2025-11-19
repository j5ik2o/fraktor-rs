//! System package.
//!
//! This module contains the actor system management.

mod actor_path_handle;
mod actor_path_registry;
mod actor_ref_provider;
mod actor_ref_provider_installer;
mod actor_system_build_error;
mod actor_system_config;
mod authority_state;
mod base;
mod extended_actor_system;
mod guardian_kind;
mod local_actor_ref_provider;
mod local_actor_ref_provider_installer;
mod register_extra_top_level_error;
mod remote_authority;
mod remote_authority_error;
mod remote_watch_hook;
mod remoting_config;
mod reservation_policy;
mod root_guardian_actor;
mod system_guardian_actor;
mod system_guardian_protocol;
mod system_state;

pub use actor_path_handle::ActorPathHandle;
pub use actor_path_registry::ActorPathRegistry;
pub use actor_ref_provider::ActorRefProvider;
pub use actor_ref_provider_installer::ActorRefProviderInstaller;
pub use actor_system_build_error::ActorSystemBuildError;
pub use actor_system_config::{ActorSystemConfig, ActorSystemConfigGeneric};
pub use authority_state::AuthorityState;
pub use base::{ActorSystem, ActorSystemGeneric};
pub use extended_actor_system::{ExtendedActorSystem, ExtendedActorSystemGeneric};
pub use guardian_kind::GuardianKind;
pub use local_actor_ref_provider::LocalActorRefProviderGeneric;
pub use local_actor_ref_provider_installer::LocalActorRefProviderInstaller;
pub use register_extra_top_level_error::RegisterExtraTopLevelError;
pub use remote_authority::{RemoteAuthorityManager, RemoteAuthorityManagerGeneric};
pub use remote_authority_error::RemoteAuthorityError;
pub use remote_watch_hook::RemoteWatchHook;
pub use remoting_config::RemotingConfig;
pub use reservation_policy::ReservationPolicy;
pub(crate) use root_guardian_actor::RootGuardianActor;
pub(crate) use system_guardian_actor::SystemGuardianActor;
pub use system_guardian_protocol::SystemGuardianProtocol;
pub use system_state::{FailureOutcome, SystemState, SystemStateGeneric};
