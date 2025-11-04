mod actor;
mod actor_adapter;

pub use actor::Actor;
pub(crate) use actor_adapter::ActorAdapter;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Context handle specialised for `StdToolbox`.
pub type ActorContext<'a> = cellactor_actor_core_rs::actor_prim::ActorContext<'a, StdToolbox>;
/// Actor reference specialised for `StdToolbox`.
pub type ActorRef = cellactor_actor_core_rs::actor_prim::actor_ref::ActorRef<StdToolbox>;
/// Child reference specialised for `StdToolbox`.
pub type ChildRef = cellactor_actor_core_rs::actor_prim::ChildRef<StdToolbox>;
