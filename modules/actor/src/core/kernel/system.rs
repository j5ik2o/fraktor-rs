//! System package.
//!
//! This module contains the actor system management.

mod actor_path_handle;
mod actor_path_registry;
mod actor_system_build_error;
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
mod register_extra_top_level_error;
mod registries;
/// Remote watch hook and authority related types.
pub mod remote;
mod reservation_policy;
/// System state related types.
pub mod state;
mod temp_actors;

mod coordinated_shutdown;
mod coordinated_shutdown_error;
mod coordinated_shutdown_id;
mod coordinated_shutdown_installer;
mod coordinated_shutdown_phase;
mod coordinated_shutdown_reason;

pub use actor_path_handle::ActorPathHandle;
pub use actor_path_registry::ActorPathRegistry;
pub use actor_system_build_error::ActorSystemBuildError;
pub use actor_system_weak::ActorSystemWeak;
pub use base::ActorSystem;
pub use coordinated_shutdown::CoordinatedShutdown;
pub use coordinated_shutdown_error::CoordinatedShutdownError;
pub use coordinated_shutdown_id::CoordinatedShutdownId;
pub use coordinated_shutdown_installer::CoordinatedShutdownInstaller;
pub use coordinated_shutdown_phase::CoordinatedShutdownPhase;
pub use coordinated_shutdown_reason::CoordinatedShutdownReason;
pub use extended_actor_system::ExtendedActorSystem;
pub use register_extra_top_level_error::RegisterExtraTopLevelError;
pub use reservation_policy::ReservationPolicy;
