//! System state related types.

// Bridge imports from parent (system) module, so that child modules can
// reference these via `super::TypeName`.
use super::{
  actor_path_registry::ActorPathRegistry,
  ask_futures::AskFutures,
  cells_shared::CellsShared,
  extensions::Extensions,
  extra_top_levels::ExtraTopLevels,
  guardian::{GuardianKind, GuardiansState},
  registries::Registries,
  remote::{
    RemoteAuthorityError, RemoteAuthorityRegistry, RemoteDeploymentHook, RemoteDeploymentHookDynShared,
    RemoteDeploymentOutcome, RemoteDeploymentRequest, RemoteWatchHook, RemoteWatchHookDynShared, RemotingConfig,
  },
  temp_actors::TempActors,
};
use crate::actor::actor_ref_provider::{
  ActorRefProvider, ActorRefProviderCaller, ActorRefProviderCallers, ActorRefProviderHandleShared, ActorRefProviders,
};

mod authority_state;
mod dispatch_mailbox_registry;
mod event_logging_registry;
mod guardian_cell_registry;
mod identity_path_registry;
mod path_identity;
mod remote_provider_registry;
mod runtime_support_registry;
mod scheduler_lifecycle_registry;
/// Shared, mutable state owned by the actor system.
pub mod system_state;
mod system_state_shared;
mod system_state_weak;

pub use authority_state::AuthorityState;
pub use system_state_shared::SystemStateShared;
pub use system_state_weak::SystemStateWeak;
