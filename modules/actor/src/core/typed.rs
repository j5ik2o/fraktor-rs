//! Typed interface wrappers around the untyped runtime.

/// Typed actor primitives (actors, contexts, references).
pub mod actor;
/// Typed actor reference wrapper promoted to the typed root.
mod actor_ref;
/// Actor-ref serialization extension for typed APIs.
mod actor_ref_resolver;
/// Typed behavior representation.
mod behavior;
/// Cross-cutting concern interceptor for typed behaviors.
mod behavior_interceptor;
/// Messages and signals delivered to typed behaviors (Pekko MessageAndSignals).
pub mod message_and_signals;
/// Point-to-point reliable delivery between a producer and consumer actor.
pub mod delivery;
/// Dispatcher registry — resolves thread-pool executors from a selector (placeholder).
mod dispatchers;
/// Dispatcher selection strategy for typed props.
mod dispatcher_selector;
/// DSL package for typed actor development (Behaviors, stash, timers, ask patterns).
pub mod dsl;
/// Typed event stream package for subscribing and publishing to the system event stream.
pub mod eventstream;
/// Generic setup wrapper for configuring extensions during system bootstrap.
mod extension_setup;
/// Internal implementation types (BehaviorRunner, TypedActorAdapter, scheduler internals).
pub(crate) mod internal;
/// Mailbox selection strategy for typed props.
mod mailbox_selector;
/// Message adapter primitives bridging external protocols.
pub mod message_adapter;
/// Typed props that wrap untyped props.
mod props;
/// Typed pub/sub package for topic actors and commands.
pub mod pubsub;
/// Typed receptionist package for service discovery primitives.
pub mod receptionist;
/// Common recipient abstraction for typed and untyped actor references.
mod recipient_ref;
/// Pekko-inspired spawn protocol for typed actors.
mod spawn_protocol;
/// Typed actor system interface.
mod system;
pub use actor_ref::TypedActorRef;
pub use actor_ref_resolver::ActorRefResolver;
pub use behavior::Behavior;
pub use behavior_interceptor::BehaviorInterceptor;
pub use dispatcher_selector::DispatcherSelector;
pub use extension_setup::ExtensionSetup;
pub use mailbox_selector::MailboxSelector;
pub use props::TypedProps;
pub use recipient_ref::RecipientRef;
pub use spawn_protocol::SpawnProtocol;
pub use system::TypedActorSystem;
#[cfg(test)]
mod tests;
