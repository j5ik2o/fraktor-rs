//! Typed interface wrappers around the untyped runtime.

/// Typed actor primitives (actors, contexts, references).
pub mod actor_prim;
/// Typed behavior builders that wrap untyped props.
mod props;
/// Typed actor system interface.
mod system;
/// Internal adapter between typed and untyped actors.
mod typed_actor_adapter;

pub use props::{TypedProps, TypedPropsGeneric};
pub use system::{TypedActorSystem, TypedActorSystemGeneric};

#[cfg(test)]
mod tests;
