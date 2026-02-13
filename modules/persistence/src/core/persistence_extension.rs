//! Persistence extension for actor systems.

#[cfg(test)]
mod tests;

use alloc::format;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  extension::Extension,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::sync_mutex_like::SyncMutexLike,
};

use crate::core::{
  journal::Journal, journal_actor::JournalActor, persistence_error::PersistenceError, snapshot_actor::SnapshotActor,
  snapshot_store::SnapshotStore,
};

/// Persistence extension type alias for the default toolbox.
pub type PersistenceExtension = PersistenceExtensionGeneric<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>;

/// Extension providing access to journal and snapshot actors.
#[derive(Clone)]
pub struct PersistenceExtensionGeneric<TB: RuntimeToolbox + 'static> {
  journal_actor:  ActorRefGeneric<TB>,
  snapshot_actor: ActorRefGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> PersistenceExtensionGeneric<TB> {
  /// Creates a new persistence extension for the given actor system.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::MessagePassing` when actor creation fails.
  pub fn new<J, S>(system: &ActorSystemGeneric<TB>, journal: J, snapshot_store: S) -> Result<Self, PersistenceError>
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
    let journal_actor = spawn_system_actor(system, "journal", move || JournalActorWrapper::new(journal.clone()))?;
    let snapshot_actor =
      spawn_system_actor(system, "snapshot", move || SnapshotActorWrapper::new(snapshot_store.clone()))?;
    Ok(Self { journal_actor, snapshot_actor })
  }

  /// Returns the journal actor reference.
  #[must_use]
  pub(crate) fn journal_actor_ref(&self) -> ActorRefGeneric<TB> {
    self.journal_actor.clone()
  }

  /// Returns the snapshot actor reference.
  #[must_use]
  pub(crate) fn snapshot_actor_ref(&self) -> ActorRefGeneric<TB> {
    self.snapshot_actor.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Extension<TB> for PersistenceExtensionGeneric<TB> {}

fn spawn_system_actor<TB, A>(
  system: &ActorSystemGeneric<TB>,
  name: &str,
  factory: impl FnMut() -> A + Send + Sync + 'static,
) -> Result<ActorRefGeneric<TB>, PersistenceError>
where
  TB: RuntimeToolbox + 'static,
  A: Actor<TB> + Sync + 'static, {
  let props = PropsGeneric::from_fn(factory).with_name(name);
  let child = system
    .extended()
    .spawn_system_actor(&props)
    .map_err(|error| PersistenceError::MessagePassing(format!("spawn error: {error:?}")))?;
  Ok(child.actor_ref().clone())
}

struct JournalActorWrapper<J: Journal, TB: RuntimeToolbox + 'static> {
  inner: ToolboxMutex<JournalActor<J, TB>, TB>,
}

impl<J: Journal, TB: RuntimeToolbox + 'static> JournalActorWrapper<J, TB>
where
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static,
{
  fn new(journal: J) -> Self {
    Self { inner: <TB::MutexFamily as SyncMutexFamily>::create(JournalActor::new(journal)) }
  }
}

impl<J: Journal, TB: RuntimeToolbox + 'static> Actor<TB> for JournalActorWrapper<J, TB>
where
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static,
{
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    self.inner.lock().receive(ctx, message)
  }
}

struct SnapshotActorWrapper<S: SnapshotStore, TB: RuntimeToolbox + 'static> {
  inner: ToolboxMutex<SnapshotActor<S, TB>, TB>,
}

impl<S: SnapshotStore, TB: RuntimeToolbox + 'static> SnapshotActorWrapper<S, TB>
where
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static,
{
  fn new(snapshot_store: S) -> Self {
    Self { inner: <TB::MutexFamily as SyncMutexFamily>::create(SnapshotActor::new(snapshot_store)) }
  }
}

impl<S: SnapshotStore, TB: RuntimeToolbox + 'static> Actor<TB> for SnapshotActorWrapper<S, TB>
where
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static,
{
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    self.inner.lock().receive(ctx, message)
  }
}
