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

pub use behavior::Behavior;
pub use behavior_signal::BehaviorSignal;
pub use behaviors::Behaviors;
pub use props::{TypedProps, TypedPropsGeneric};
pub use system::{TypedActorSystem, TypedActorSystemGeneric};

#[cfg(test)]
mod tests;
