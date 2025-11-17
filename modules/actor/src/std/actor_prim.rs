mod actor;
mod actor_adapter;

pub use actor::Actor;
pub use actor_adapter::ActorAdapter;
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Context handle specialised for `StdToolbox`.
pub type ActorContext<'a> = crate::core::actor_prim::ActorContextGeneric<'a, StdToolbox>;
/// Actor reference specialised for `StdToolbox`.
pub type ActorRef = crate::core::actor_prim::actor_ref::ActorRefGeneric<StdToolbox>;
/// Child reference specialised for `StdToolbox`.
pub type ChildRef = crate::core::actor_prim::ChildRefGeneric<StdToolbox>;
