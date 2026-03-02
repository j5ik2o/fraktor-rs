mod actor_adapter;
mod actor_context;
mod actor_lifecycle;

pub use actor_adapter::ActorAdapter;
pub use actor_context::ActorContext;
pub use actor_lifecycle::Actor;

/// Actor reference specialised for `StdToolbox`.
pub type ActorRef = crate::core::actor::actor_ref::ActorRef;
/// Child reference specialised for `StdToolbox`.
pub type ChildRef = crate::core::actor::ChildRef;
