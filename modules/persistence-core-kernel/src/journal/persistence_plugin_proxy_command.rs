//! Commands for the persistence plugin proxy actor.

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;

/// Commands handled by
/// [`PersistencePluginProxyActor`](crate::journal::PersistencePluginProxyActor).
#[derive(Clone, Debug)]
pub enum PersistencePluginProxyCommand {
  /// Sets the actor that receives journal protocol messages.
  SetJournalTarget {
    /// Journal actor target.
    target: ActorRef,
  },
  /// Sets the actor that receives snapshot protocol messages.
  SetSnapshotTarget {
    /// Snapshot actor target.
    target: ActorRef,
  },
}
