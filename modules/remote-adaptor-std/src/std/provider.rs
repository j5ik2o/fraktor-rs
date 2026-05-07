//! Adapter-side actor ref provider that performs the loopback / remote
//! dispatch demanded by design Decision 3-C.
//!
//! `remote-core` ships a **remote-only**
//! [`fraktor_remote_core_rs::core::provider::RemoteActorRefProvider`]: it never resolves a local
//! `ActorPath`. The adapter installs [`dispatch::StdRemoteActorRefProvider`] in front of it,
//! inspects every incoming `ActorPath`, and forwards local-bound traffic to actor-core's
//! `LocalActorRefProvider` while passing remote-bound traffic to the core
//! provider.

#[cfg(test)]
mod tests;

mod dispatch;
mod path_remote_actor_ref_provider;
mod provider_dispatch_error;
mod remote_actor_ref_sender;
mod remote_routee_expansion;
mod remote_routee_expansion_error;
mod std_remote_actor_ref_provider_installer;

pub use dispatch::StdRemoteActorRefProvider;
pub use path_remote_actor_ref_provider::PathRemoteActorRefProvider;
pub use provider_dispatch_error::StdRemoteActorRefProviderError;
pub use remote_actor_ref_sender::RemoteActorRefSender;
pub use remote_routee_expansion::RemoteRouteeExpansion;
pub use remote_routee_expansion_error::RemoteRouteeExpansionError;
pub use std_remote_actor_ref_provider_installer::StdRemoteActorRefProviderInstaller;
