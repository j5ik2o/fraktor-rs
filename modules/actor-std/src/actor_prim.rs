mod actor;
mod actor_adapter;

pub use actor::Actor;
pub(crate) use actor_adapter::ActorAdapter;
use fraktor_utils_core_rs::std::runtime_toolbox::StdToolbox;

/// Context handle specialised for `StdToolbox`.
pub type ActorContext<'a> = fraktor_actor_core_rs::actor_prim::ActorContextGeneric<'a, StdToolbox>;
/// Actor reference specialised for `StdToolbox`.
pub type ActorRef = fraktor_actor_core_rs::actor_prim::actor_ref::ActorRefGeneric<StdToolbox>;
/// Child reference specialised for `StdToolbox`.
pub type ChildRef = fraktor_actor_core_rs::actor_prim::ChildRefGeneric<StdToolbox>;
