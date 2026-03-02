mod actor_adapter;
mod actor_context;
mod actor_lifecycle;

pub use actor_adapter::ActorAdapter;
pub use actor_context::ActorContext;
pub use actor_lifecycle::Actor;

/// Actor reference for the standard runtime.
pub type ActorRef = crate::core::actor::actor_ref::ActorRef;
/// Child reference for the standard runtime.
pub type ChildRef = crate::core::actor::ChildRef;
