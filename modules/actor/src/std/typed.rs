//! High-level typed actor bindings for the standard fraktor runtime.

/// Core typed actor primitives including actors, contexts, and references.
pub mod actor;
mod behaviors;
mod props;
mod system;

pub use behaviors::Behaviors;
pub use props::TypedProps;
pub use system::TypedActorSystem;
