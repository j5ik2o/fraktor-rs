//! Typed interface wrappers around the untyped runtime.

/// Pekko-compatible alias for [`ExtensionSetup`].
mod abstract_extension_setup;
/// Typed actor primitives (actors, contexts, references).
pub mod actor;
/// Typed actor reference wrapper promoted to the typed root.
mod actor_ref;
/// Actor-ref serialization extension for typed APIs.
mod actor_ref_resolver;
/// Setup wrapper for replacing the default actor-ref resolver extension.
mod actor_ref_resolver_setup;
/// Typed metadata tags facade for actor props.
mod actor_tags;
/// Typed backoff supervision strategy facade.
mod backoff_supervisor_strategy;
/// Typed behavior representation.
mod behavior;
/// Cross-cutting concern interceptor for typed behaviors.
mod behavior_interceptor;
/// Point-to-point reliable delivery between a producer and consumer actor.
pub mod delivery;
/// Dispatcher selection strategy for typed props.
mod dispatcher_selector;
/// Typed dispatcher lookup facade.
mod dispatchers;
/// DSL package for typed actor development (Behaviors, stash, timers, ask patterns).
pub mod dsl;
/// Typed event stream package for subscribing and publishing to the system event stream.
pub mod eventstream;
/// Generic setup wrapper for configuring extensions during system bootstrap.
mod extension_setup;
/// Internal implementation types (BehaviorRunner, TypedActorAdapter, scheduler internals).
mod internal;
/// Logging options for typed behavior helpers.
mod log_options;
/// Mailbox selection strategy for typed props.
mod mailbox_selector;
/// Message adapter primitives bridging external protocols.
pub mod message_adapter;
/// Messages and signals delivered to typed behaviors (Pekko MessageAndSignals).
pub mod message_and_signals;
/// Typed props that wrap untyped props.
mod props;
/// Typed pub/sub package for topic actors and commands.
pub mod pubsub;
/// Typed receptionist package for service discovery primitives.
pub mod receptionist;
/// Common recipient abstraction for typed and untyped actor references.
mod recipient_ref;
/// Typed restart supervision strategy facade.
mod restart_supervisor_strategy;
/// Typed scheduler facade.
mod scheduler;
/// Pekko-inspired spawn protocol for typed actors.
mod spawn_protocol;
/// Typed supervisor strategy factory facade.
mod supervisor_strategy;
/// Typed actor system interface.
mod system;
/// System-level log handle for typed actor systems.
mod typed_actor_system_log;
/// Immutable metadata snapshot for typed actor systems.
mod typed_actor_system_settings;
pub use abstract_extension_setup::AbstractExtensionSetup;
pub use actor_ref::TypedActorRef;
pub use actor_ref_resolver::ActorRefResolver;
pub use actor_ref_resolver_setup::ActorRefResolverSetup;
pub use actor_tags::ActorTags;
pub use backoff_supervisor_strategy::BackoffSupervisorStrategy;
pub use behavior::Behavior;
pub use behavior_interceptor::BehaviorInterceptor;
pub use dispatcher_selector::DispatcherSelector;
pub use dispatchers::Dispatchers;
pub use extension_setup::ExtensionSetup;
pub use log_options::LogOptions;
pub use mailbox_selector::MailboxSelector;
pub use props::TypedProps;
pub use recipient_ref::RecipientRef;
pub use restart_supervisor_strategy::RestartSupervisorStrategy;
pub use scheduler::Scheduler;
pub use spawn_protocol::SpawnProtocol;
pub use supervisor_strategy::SupervisorStrategy;
pub use system::TypedActorSystem;
pub use typed_actor_system_log::TypedActorSystemLog;
pub use typed_actor_system_settings::TypedActorSystemSettings;
#[cfg(test)]
mod tests;
