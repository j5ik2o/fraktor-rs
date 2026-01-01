//! Public entry points for persistent actor construction.

use fraktor_actor_rs::core::{
  actor::{ActorContextGeneric, actor_ref::ActorRefGeneric},
  props::PropsGeneric,
  spawn::SpawnError,
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{persistent_actor::PersistentActor, persistent_actor_adapter::PersistentActorAdapter};

/// Builds props for a persistent actor, applying the adapter internally.
#[must_use]
pub fn persistent_props<TB, F, A>(mut factory: F) -> PropsGeneric<TB>
where
  TB: RuntimeToolbox + 'static,
  F: FnMut() -> A + Send + Sync + 'static,
  A: PersistentActor<TB> + Sync + 'static, {
  PropsGeneric::from_fn(move || PersistentActorAdapter::new(factory()))
}

/// Spawns a persistent actor as a child of the provided context.
///
/// # Errors
///
/// Returns [`SpawnError`] when the child actor cannot be spawned.
pub fn spawn_persistent<TB>(
  ctx: &ActorContextGeneric<'_, TB>,
  props: &PropsGeneric<TB>,
) -> Result<ActorRefGeneric<TB>, SpawnError>
where
  TB: RuntimeToolbox + 'static, {
  ctx.spawn_child(props).map(|child| child.actor_ref().clone())
}
