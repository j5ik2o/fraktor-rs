//! System package.
//!
//! This module contains the actor system management.

mod actor_path_handle;
mod actor_path_registry;
mod actor_system_build_error;
mod actor_system_config;
mod actor_system_weak;
mod ask_futures;
mod base;
mod cells;
mod cells_shared;
mod extended_actor_system;
mod extensions;
mod extra_top_levels;
/// Guardian actor related types.
pub mod guardian;
/// Actor reference provider related types.
pub mod provider;
mod register_extension_error;
mod register_extra_top_level_error;
mod registries;
/// Remote watch hook and authority related types.
pub mod remote;
mod reservation_policy;
/// System state related types.
pub mod state;
mod temp_actors;

pub use actor_path_handle::ActorPathHandle;
pub use actor_path_registry::ActorPathRegistry;
pub use actor_system_build_error::ActorSystemBuildError;
pub use actor_system_config::{ActorSystemConfig, ActorSystemConfigGeneric};
pub use actor_system_weak::{ActorSystemWeak, ActorSystemWeakGeneric};
pub use base::{ActorSystem, ActorSystemGeneric};
pub use extended_actor_system::{ExtendedActorSystem, ExtendedActorSystemGeneric};
pub use register_extension_error::RegisterExtensionError;
pub use register_extra_top_level_error::RegisterExtraTopLevelError;
pub use reservation_policy::ReservationPolicy;
