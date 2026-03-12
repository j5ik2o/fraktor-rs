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
/// Dispatcher selection strategy for typed props.
mod dispatcher_selector;
/// Generic setup wrapper for configuring extensions during system bootstrap.
mod extension_setup;
/// Type-specific failure handler for supervision DSL.
mod failure_handler;
/// Minimal FSM DSL builder for typed behaviors.
mod fsm_builder;
/// Builder for configuring and constructing group routers.
mod group_router_builder;
/// Snapshot of actor references registered under a service key.
mod listing;
/// Mailbox selection strategy for typed props.
mod mailbox_selector;
/// Message adapter primitives bridging external protocols.
pub mod message_adapter;
/// Builder for configuring and constructing pool routers.
mod pool_router_builder;
/// Typed props that wrap untyped props.
mod props;
/// Internal configuration state for actor receive timeouts.
mod receive_timeout_config;
/// Receptionist actor for service discovery.
mod receptionist;
/// Command messages for the Receptionist.
mod receptionist_command;
/// Common recipient abstraction for typed and untyped actor references.
mod recipient_ref;
/// Pekko-inspired router factories.
mod routers;
/// Typed scheduler facade mirroring the untyped API.
pub mod scheduler;
/// Type-safe service key for actor discovery.
mod service_key;
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
/// Typed pub/sub topic actor.
mod topic;
/// Commands accepted by the typed topic actor.
mod topic_command;
/// Snapshot returned by typed topic stats queries.
mod topic_stats;
/// Internal adapter between typed and untyped actors.
mod typed_actor_adapter;
/// Typed ask error classification.
mod typed_ask_error;
/// Typed ask future helpers.
mod typed_ask_future;
/// Typed ask response handle.
mod typed_ask_response;
/// Unhandled message event for monitoring.
mod unhandled_message_event;

pub use actor_ref_resolver::ActorRefResolver;
pub use actor_ref_resolver_id::ActorRefResolverId;
pub use behavior::Behavior;
pub use behavior_interceptor::BehaviorInterceptor;
pub use behavior_signal::BehaviorSignal;
pub use behavior_signal_interceptor::BehaviorSignalInterceptor;
pub use behaviors::Behaviors;
pub use dispatcher_selector::DispatcherSelector;
pub use extension_setup::ExtensionSetup;
pub use failure_handler::FailureHandler;
pub use fsm_builder::FsmBuilder;
pub use group_router_builder::GroupRouterBuilder;
pub use listing::Listing;
pub use mailbox_selector::MailboxSelector;
pub use pool_router_builder::PoolRouterBuilder;
pub use props::TypedProps;
pub use receptionist::{Receptionist, SYSTEM_RECEPTIONIST_TOP_LEVEL};
pub use receptionist_command::ReceptionistCommand;
pub use recipient_ref::RecipientRef;
pub use routers::Routers;
pub use service_key::ServiceKey;
pub use spawn_protocol::SpawnProtocol;
pub use stash_buffer::StashBuffer;
pub use status_reply::StatusReply;
pub use status_reply_error::StatusReplyError;
pub use supervise::Supervise;
pub use system::TypedActorSystem;
pub use timer_key::TimerKey;
pub use timer_scheduler::{TimerScheduler, TimerSchedulerShared};
pub use topic::Topic;
pub use topic_command::TopicCommand;
pub use topic_stats::TopicStats;
pub use typed_ask_error::TypedAskError;
pub use typed_ask_future::TypedAskFuture;
pub use typed_ask_response::TypedAskResponse;
pub use unhandled_message_event::UnhandledMessageEvent;

#[cfg(test)]
mod tests;
