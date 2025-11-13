//! Typed interface wrappers around the untyped runtime.

/// Typed actor primitives (actors, contexts, references).
pub mod actor_prim;
/// Typed behavior representation.
mod behavior;
/// Internal executor that drives behavior state machines.
mod behavior_runner;
/// Typed behavior signals forwarded from the runtime.
mod behavior_signal;
/// Functional behavior builders inspired by Fraktor.
mod behaviors;
/// Message adapter primitives bridging external protocols.
pub mod message_adapter;
/// Typed props that wrap untyped props.
mod props;
/// Typed scheduler facade mirroring the untyped API.
mod scheduler;
/// Builder for assigning supervisor strategies to behaviors.
mod supervise;
/// Typed actor system interface.
mod system;
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

pub use actor_prim::{
  TypedActor, TypedActorContext, TypedActorContextGeneric, TypedActorRef, TypedActorRefGeneric, TypedChildRef,
  TypedChildRefGeneric,
};
pub use behavior::Behavior;
pub use behavior_signal::BehaviorSignal;
pub use behaviors::Behaviors;
pub use message_adapter::{AdapterError, AdapterFailure, AdapterOutcome, AdapterPayload, MessageAdapterRegistry};
pub use props::{TypedProps, TypedPropsGeneric};
pub use scheduler::TypedScheduler;
pub use supervise::Supervise;
pub use system::{TypedActorSystem, TypedActorSystemGeneric};
pub use typed_ask_error::TypedAskError;
pub use typed_ask_future::{TypedAskFuture, TypedAskFutureGeneric};
pub use typed_ask_response::{TypedAskResponse, TypedAskResponseGeneric};
pub use unhandled_message_event::UnhandledMessageEvent;

#[cfg(test)]
mod tests;
