//! Remote watch hook and authority related types.

use crate::core::kernel::actor::actor_ref_provider::ActorRefProvider;

mod noop_remote_watch_hook;
mod remote_authority_error;
mod remote_authority_registry;
mod remote_watch_hook;
mod remote_watch_hook_dyn_shared;
mod remote_watch_hook_handle;
mod remote_watch_hook_handle_shared;
mod remote_watch_hook_shared_factory;
mod remoting_config;

pub use remote_authority_error::RemoteAuthorityError;
pub use remote_authority_registry::RemoteAuthorityRegistry;
pub use remote_watch_hook::RemoteWatchHook;
pub(crate) use remote_watch_hook_dyn_shared::RemoteWatchHookDynShared;
pub use remote_watch_hook_handle::RemoteWatchHookHandle;
pub use remote_watch_hook_handle_shared::RemoteWatchHookHandleShared;
pub use remote_watch_hook_shared_factory::RemoteWatchHookHandleSharedFactory;
pub use remoting_config::RemotingConfig;
