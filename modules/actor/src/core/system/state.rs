//! System state related types.

// Bridge imports from parent (system) module, so that child modules can
// reference these via `super::TypeName`.
use super::{
  actor_path_registry::ActorPathRegistry,
  ask_futures::AskFuturesGeneric,
  cells_shared::CellsSharedGeneric,
  extensions::ExtensionsGeneric,
  extra_top_levels::ExtraTopLevelsGeneric,
  guardian::{GuardianKind, GuardiansState},
  provider::{
    ActorRefProvider, ActorRefProviderCaller, ActorRefProviderCallersGeneric, ActorRefProviderHandle,
    ActorRefProviderSharedGeneric, ActorRefProvidersGeneric,
  },
  registries::RegistriesGeneric,
  remote::{
    RemoteAuthorityError, RemoteAuthorityRegistryGeneric, RemoteWatchHook, RemoteWatchHookDynSharedGeneric,
    RemotingConfig,
  },
  temp_actors::TempActorsGeneric,
};

mod authority_state;
#[cfg(any(test, feature = "test-support"))]
pub(crate) mod booting_state;
#[cfg(any(test, feature = "test-support"))]
pub(crate) mod running_state;
/// Shared, mutable state owned by the actor system.
pub mod system_state;
mod system_state_shared;
mod system_state_weak;

pub use authority_state::AuthorityState;
pub use system_state_shared::{SystemStateShared, SystemStateSharedGeneric};
pub use system_state_weak::SystemStateWeakGeneric;
