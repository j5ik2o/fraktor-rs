//! Public entry points for persistent actor construction.

#[cfg(test)]
mod tests;

use fraktor_actor_core_rs::actor::{ActorContext, ChildRef, actor_ref::ActorRef, props::Props, spawn::SpawnError};

use crate::core::{persistent_actor::PersistentActor, persistent_actor_adapter::PersistentActorAdapter};

/// Builds props for a persistent actor, applying the adapter internally.
#[must_use]
pub fn persistent_props<F, A>(mut factory: F) -> Props
where
  F: FnMut() -> A + Send + Sync + 'static,
  A: PersistentActor + Sync + 'static, {
  Props::from_fn(move || PersistentActorAdapter::new(factory())).with_stash_mailbox()
}

/// Spawns a persistent actor as a child of the provided context.
///
/// # Errors
///
/// Returns [`SpawnError`] when the child actor cannot be spawned.
pub fn spawn_persistent(ctx: &mut ActorContext<'_>, props: &Props) -> Result<ActorRef, SpawnError> {
  ctx.spawn_child(props).map(ChildRef::into_actor_ref)
}
