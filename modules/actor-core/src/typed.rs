//! Typed interface wrappers around the untyped runtime.

/// Typed actor primitives (actors, contexts, references).
pub mod actor_prim;
/// Typed behavior builders that wrap untyped props.
mod behavior;
/// Internal adapter between typed and untyped actors.
mod behavior_adapter;
/// Typed actor system interface.
mod system;

pub use behavior::{Behavior, BehaviorGeneric};
pub use system::{TypedActorSystem, TypedActorSystemGeneric};

#[cfg(test)]
mod tests;
