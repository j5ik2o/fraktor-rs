//! Remote watch hook and authority related types.

use super::provider::ActorRefProvider;

mod noop_remote_watch_hook;
mod remote_authority_error;
mod remote_authority_registry;
mod remote_watch_hook;
mod remote_watch_hook_dyn_shared;
mod remote_watch_hook_handle;
mod remote_watch_hook_shared;
mod remoting_config;

pub use remote_authority_error::RemoteAuthorityError;
pub use remote_authority_registry::{RemoteAuthorityRegistry, RemoteAuthorityRegistryGeneric};
pub use remote_watch_hook::RemoteWatchHook;
pub(crate) use remote_watch_hook_dyn_shared::RemoteWatchHookDynSharedGeneric;
pub use remote_watch_hook_handle::RemoteWatchHookHandle;
pub use remote_watch_hook_shared::RemoteWatchHookShared;
pub use remoting_config::RemotingConfig;
