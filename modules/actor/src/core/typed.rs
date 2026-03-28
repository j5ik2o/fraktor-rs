//! Typed interface wrappers around the untyped runtime.

/// Typed actor primitives (actors, contexts, references).
pub mod actor;
/// Actor-ref serialization extension for typed APIs.
mod actor_ref_resolver;
/// Identifier used to register the actor-ref resolver extension.
mod actor_ref_resolver_id;
/// Typed behavior representation.
mod behavior;
/// Cross-cutting concern interceptor for typed behaviors.
mod behavior_interceptor;
/// Internal executor that drives behavior state machines.
mod behavior_runner;
/// Typed behavior signals forwarded from the runtime.
mod behavior_signal;
/// Signal-only interceptor specialization for typed behaviors.
mod behavior_signal_interceptor;
/// Functional behavior builders inspired by Fraktor.
mod behaviors;
/// Pekko-compatible death pact exception for typed actors.
mod death_pact_exception;
/// Point-to-point reliable delivery between a producer and consumer actor.
pub mod delivery;
/// Dispatcher selection strategy for typed props.
mod dispatcher_selector;
/// Generic setup wrapper for configuring extensions during system bootstrap.
mod extension_setup;
/// Type-specific failure handler for supervision DSL.
mod failure_handler;
/// Minimal FSM DSL builder for typed behaviors.
mod fsm_builder;
/// Mailbox selection strategy for typed props.
mod mailbox_selector;
/// Message adapter primitives bridging external protocols.
pub mod message_adapter;
/// Typed props that wrap untyped props.
mod props;
/// Typed pub/sub package for topic actors and commands.
pub mod pubsub;
/// Internal configuration state for actor receive timeouts.
mod receive_timeout_config;
/// Typed receptionist package for service discovery primitives.
pub mod receptionist;
/// Common recipient abstraction for typed and untyped actor references.
mod recipient_ref;
/// Typed routing package for routers, builders, and resizers.
pub mod routing;
/// Typed scheduler facade mirroring the untyped API.
pub mod scheduler;
/// Pekko-inspired spawn protocol for typed actors.
mod spawn_protocol;
/// Bounded stash helper used by `Behaviors::with_stash`.
mod stash_buffer;
/// Status-aware reply type for typed ask patterns.
mod status_reply;
/// Error type for status-aware ask responses.
mod status_reply_error;
/// Builder for assigning supervisor strategies to behaviors.
mod supervise;
/// Typed actor system interface.
mod system;
/// Key type for identifying timers.
mod timer_key;
/// Actor-scoped timer management.
mod timer_scheduler;
/// Internal adapter between typed and untyped actors.
mod typed_actor_adapter;
/// Typed ask error classification.
mod typed_ask_error;
/// Typed ask future helpers.
mod typed_ask_future;
/// Typed ask response handle.
mod typed_ask_response;

pub use actor_ref_resolver::ActorRefResolver;
pub use actor_ref_resolver_id::ActorRefResolverId;
pub use behavior::Behavior;
pub use behavior_interceptor::BehaviorInterceptor;
pub use behavior_signal::BehaviorSignal;
pub use behavior_signal_interceptor::BehaviorSignalInterceptor;
pub use behaviors::Behaviors;
pub use death_pact_exception::DeathPactException;
pub use dispatcher_selector::DispatcherSelector;
pub use extension_setup::ExtensionSetup;
pub use failure_handler::FailureHandler;
pub use fsm_builder::FsmBuilder;
pub use mailbox_selector::MailboxSelector;
pub use props::TypedProps;
pub use recipient_ref::RecipientRef;
pub use spawn_protocol::SpawnProtocol;
pub use stash_buffer::StashBuffer;
pub use status_reply::StatusReply;
pub use status_reply_error::StatusReplyError;
pub use supervise::Supervise;
pub use system::TypedActorSystem;
pub use timer_key::TimerKey;
pub use timer_scheduler::{TimerScheduler, TimerSchedulerShared};
pub use typed_ask_error::TypedAskError;
pub use typed_ask_future::TypedAskFuture;
pub use typed_ask_response::TypedAskResponse;
#[cfg(test)]
mod tests;
