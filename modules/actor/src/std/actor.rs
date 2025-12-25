mod actor_adapter;
mod actor_context;
mod actor_lifecycle;

pub use actor_adapter::ActorAdapter;
pub use actor_context::ActorContext;
pub use actor_lifecycle::Actor;
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Actor reference specialised for `StdToolbox`.
pub type ActorRef = crate::core::actor::actor_ref::ActorRefGeneric<StdToolbox>;
/// Child reference specialised for `StdToolbox`.
pub type ChildRef = crate::core::actor::ChildRefGeneric<StdToolbox>;
