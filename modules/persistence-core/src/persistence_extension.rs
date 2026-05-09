//! Persistence extension for actor systems.

#[cfg(test)]
mod tests;

use alloc::format;

use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, actor_ref::ActorRef, error::ActorError, extension::Extension, messaging::AnyMessageView,
    props::Props,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedLock};

use crate::{
  journal::Journal, journal_actor::JournalActor, persistence_error::PersistenceError, snapshot_actor::SnapshotActor,
  snapshot_store::SnapshotStore,
};

/// Extension providing access to journal and snapshot actors.
#[derive(Clone)]
pub struct PersistenceExtension {
  journal_actor:  ActorRef,
  snapshot_actor: ActorRef,
}

impl PersistenceExtension {
  /// Creates a new persistence extension for the given actor system.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::MessagePassing` when actor creation fails.
  pub fn new<J, S>(system: &ActorSystem, journal: J, snapshot_store: S) -> Result<Self, PersistenceError>
  where
    J: Journal + Clone + Send + Sync + 'static,
    S: SnapshotStore + Clone + Send + Sync + 'static,
    for<'a> J::WriteFuture<'a>: Send + 'static,
    for<'a> J::ReplayFuture<'a>: Send + 'static,
    for<'a> J::DeleteFuture<'a>: Send + 'static,
    for<'a> J::HighestSeqNrFuture<'a>: Send + 'static,
    for<'a> S::SaveFuture<'a>: Send + 'static,
    for<'a> S::LoadFuture<'a>: Send + 'static,
    for<'a> S::DeleteOneFuture<'a>: Send + 'static,
    for<'a> S::DeleteManyFuture<'a>: Send + 'static, {
    let journal_actor = spawn_system_actor(system, "journal", move || JournalActorWrapper::<J>::new(journal.clone()))?;
    let snapshot_actor =
      spawn_system_actor(system, "snapshot", move || SnapshotActorWrapper::<S>::new(snapshot_store.clone()))?;
    Ok(Self { journal_actor, snapshot_actor })
  }

  /// Returns the journal actor reference.
  #[must_use]
  pub(crate) fn journal_actor_ref(&self) -> ActorRef {
    self.journal_actor.clone()
  }

  /// Returns the snapshot actor reference.
  #[must_use]
  pub(crate) fn snapshot_actor_ref(&self) -> ActorRef {
    self.snapshot_actor.clone()
  }
}

impl Extension for PersistenceExtension {}

fn spawn_system_actor<A>(
  system: &ActorSystem,
  name: &str,
  factory: impl FnMut() -> A + Send + Sync + 'static,
) -> Result<ActorRef, PersistenceError>
where
  A: Actor + Sync + 'static, {
  let props = Props::from_fn(factory).with_name(name);
  let child = system
    .extended()
    .spawn_system_actor(&props)
    .map_err(|error| PersistenceError::MessagePassing(format!("spawn error: {error:?}")))?;
  Ok(child.into_actor_ref())
}

struct JournalActorWrapper<J: Journal> {
  inner: SharedLock<JournalActor<J>>,
}

impl<J: Journal> JournalActorWrapper<J>
where
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static,
{
  fn new(journal: J) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(JournalActor::new(journal)) }
  }
}

impl<J: Journal> Actor for JournalActorWrapper<J>
where
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static,
{
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.inner.with_lock(|inner| inner.receive(ctx, message))
  }
}

struct SnapshotActorWrapper<S: SnapshotStore> {
  inner: SharedLock<SnapshotActor<S>>,
}

impl<S: SnapshotStore> SnapshotActorWrapper<S>
where
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static,
{
  fn new(snapshot_store: S) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(SnapshotActor::new(snapshot_store)) }
  }
}

impl<S: SnapshotStore> Actor for SnapshotActorWrapper<S>
where
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static,
{
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.inner.with_lock(|inner| inner.receive(ctx, message))
  }
}
