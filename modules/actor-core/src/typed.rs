//! Typed interface wrappers around the untyped runtime.

/// Typed actor primitives (actors, contexts, references).
pub mod actor_prim;
/// Typed behavior builders that wrap untyped props.
pub mod behavior;
/// Internal adapter between typed and untyped actors.
pub mod behavior_adapter;
/// Typed actor system interface.
pub mod system;

#[cfg(test)]
mod tests;
