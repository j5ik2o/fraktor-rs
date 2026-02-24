//! Typed interface wrappers around the untyped runtime.

/// Typed actor primitives (actors, contexts, references).
pub mod actor;
/// Typed behavior representation.
mod behavior;
/// Cross-cutting concern interceptor for typed behaviors.
mod behavior_interceptor;
/// Internal executor that drives behavior state machines.
mod behavior_runner;
/// Typed behavior signals forwarded from the runtime.
mod behavior_signal;
/// Functional behavior builders inspired by Fraktor.
mod behaviors;
/// Message adapter primitives bridging external protocols.
pub mod message_adapter;
/// Builder for configuring and constructing pool routers.
mod pool_router_builder;
/// Typed props that wrap untyped props.
mod props;
/// Internal configuration state for actor receive timeouts.
mod receive_timeout_config;
/// Pekko-inspired router factories.
mod routers;
/// Typed scheduler facade mirroring the untyped API.
pub mod scheduler;
/// Bounded stash helper used by `Behaviors::with_stash`.
mod stash_buffer;
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
/// Unhandled message event for monitoring.
mod unhandled_message_event;

pub use behavior::Behavior;
pub use behavior_interceptor::BehaviorInterceptor;
pub use behavior_signal::BehaviorSignal;
pub use behaviors::Behaviors;
pub use pool_router_builder::{PoolRouterBuilder, PoolRouterBuilderGeneric};
pub use props::{TypedProps, TypedPropsGeneric};
pub use routers::Routers;
pub use stash_buffer::{StashBuffer, StashBufferGeneric};
pub use supervise::Supervise;
pub use system::{TypedActorSystem, TypedActorSystemGeneric};
pub use timer_key::TimerKey;
pub use timer_scheduler::{TimerScheduler, TimerSchedulerGeneric, TimerSchedulerShared};
pub use typed_ask_error::TypedAskError;
pub use typed_ask_future::{TypedAskFuture, TypedAskFutureGeneric};
pub use typed_ask_response::{TypedAskResponse, TypedAskResponseGeneric};
pub use unhandled_message_event::UnhandledMessageEvent;

#[cfg(test)]
mod tests;
