mod actor;
mod actor_adapter;
mod actor_context;

pub use actor::Actor;
pub use actor_adapter::ActorAdapter;
pub use actor_context::ActorContext;
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Actor reference specialised for `StdToolbox`.
pub type ActorRef = crate::core::actor_prim::actor_ref::ActorRefGeneric<StdToolbox>;
/// Child reference specialised for `StdToolbox`.
pub type ChildRef = crate::core::actor_prim::ChildRefGeneric<StdToolbox>;
