//! Remote-only actor ref provider port.
//!
//! The crate ships a **single** provider trait ([`RemoteActorRefProvider`])
//! that is intentionally scoped to remote paths only. Routing a local
//! `ActorPath` through `actor_ref` is a contract violation — the adapter layer
//! (Phase B) is expected to check the `ActorPath` authority and dispatch
//! local traffic to the actor-core local provider before ever touching this
//! trait. See design Decision 3-C for the full rationale.

#[cfg(test)]
mod tests;

mod path_resolver;
mod provider_error;
mod remote_actor_ref;
mod remote_actor_ref_provider;

pub use path_resolver::resolve_remote_address;
pub use provider_error::ProviderError;
pub use remote_actor_ref::RemoteActorRef;
pub use remote_actor_ref_provider::RemoteActorRefProvider;
