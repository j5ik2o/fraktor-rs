use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;
use fraktor_actor_core_typed_rs::TypedActorRef;
use fraktor_persistence_core_kernel_rs::persistent::Eventsourced;

use crate::{
  PersistenceEffectorConfig, PersistenceId, Recovery, SnapshotSelectionCriteria,
  internal::{PersistenceStoreActor, PersistenceStoreReply},
};

fn apply_event(state: &u32, event: &u32) -> u32 {
  state + event
}

#[test]
fn store_actor_recovery_uses_configured_replay_bounds() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event)
    .with_recovery(Recovery::new(20, 5));
  let reply_to = TypedActorRef::<PersistenceStoreReply<u32, u32>>::from_untyped(ActorRef::null());
  let actor = PersistenceStoreActor::new(config, reply_to);

  let recovery = actor.recovery();

  assert_eq!(recovery.to_sequence_nr(), 20);
  assert_eq!(recovery.replay_max(), 5);
}

#[test]
fn store_actor_recovery_uses_configured_snapshot_selection() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event)
    .with_recovery(Recovery::from_snapshot(SnapshotSelectionCriteria::to_sequence_nr(9)));
  let reply_to = TypedActorRef::<PersistenceStoreReply<u32, u32>>::from_untyped(ActorRef::null());
  let actor = PersistenceStoreActor::new(config, reply_to);

  let recovery = actor.recovery();

  assert_eq!(recovery.snapshot_criteria().max_sequence_nr(), 9);
}
