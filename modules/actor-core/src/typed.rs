//! Typed interface wrappers around the untyped runtime.

/// Typed actor primitives (actors, contexts, references).
pub mod actor_prim;
/// Typed behavior representation.
mod behavior;
/// Internal executor that drives behavior state machines.
mod behavior_runner;
/// Typed behavior signals forwarded from the runtime.
mod behavior_signal;
/// Functional behavior builders inspired by Pekko.
mod behaviors;
/// Typed props that wrap untyped props.
mod props;
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

pub use behavior::Behavior;
pub use behavior_signal::BehaviorSignal;
pub use behaviors::Behaviors;
pub use props::{TypedProps, TypedPropsGeneric};
pub use system::{TypedActorSystem, TypedActorSystemGeneric};
pub use typed_ask_error::TypedAskError;
pub use typed_ask_future::TypedAskFuture;
pub use typed_ask_response::{TypedAskResponse, TypedAskResponseGeneric};

#[cfg(test)]
mod tests;
