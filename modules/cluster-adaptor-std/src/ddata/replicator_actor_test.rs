use fraktor_cluster_core_kernel_rs::ddata::{Flag, FlagKey, ReadConsistency, ReplicatorSettings, WriteConsistency};

use super::{ReplicatorActor, ReplicatorGet, ReplicatorMembershipHook, ReplicatorUpdate};

#[test]
fn membership_hook_preserves_local_get_behavior() {
  let mut actor = ReplicatorActor::<Flag, u64>::new(ReplicatorSettings::new());
  actor.on_membership_event(ReplicatorMembershipHook);

  let _ = actor.handle_update(&ReplicatorUpdate::<Flag, ()>::new(flag_key(), WriteConsistency::Local), |_| {
    Ok(Flag::disabled().switch_on())
  });

  let outcome = actor.handle_get(&ReplicatorGet::<Flag, ()>::new(flag_key(), ReadConsistency::Local));
  assert!(outcome.response.is_some());
}

fn flag_key() -> FlagKey {
  FlagKey::new("flag")
}
