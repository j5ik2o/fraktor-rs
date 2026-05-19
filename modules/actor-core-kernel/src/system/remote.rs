//! Remote watch hook and authority related types.

mod noop_remote_deployment_hook;
mod noop_remote_watch_hook;
mod remote_authority_error;
mod remote_authority_registry;
mod remote_deployment_hook;
mod remote_deployment_hook_dyn_shared;
mod remote_deployment_outcome;
mod remote_deployment_request;
mod remote_watch_hook;
mod remote_watch_hook_dyn_shared;
mod remoting_config;

pub use remote_authority_error::RemoteAuthorityError;
pub use remote_authority_registry::RemoteAuthorityRegistry;
pub use remote_deployment_hook::RemoteDeploymentHook;
pub(crate) use remote_deployment_hook_dyn_shared::RemoteDeploymentHookDynShared;
pub use remote_deployment_outcome::RemoteDeploymentOutcome;
pub use remote_deployment_request::RemoteDeploymentRequest;
pub use remote_watch_hook::RemoteWatchHook;
pub(crate) use remote_watch_hook_dyn_shared::RemoteWatchHookDynShared;
pub use remoting_config::RemotingConfig;
