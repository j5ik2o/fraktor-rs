use fraktor_cluster_core_kernel_rs::ddata::{
  Flag, FlagKey, Get, ReadConsistency, ReplicatorSettings, Update, WriteConsistency,
};

use super::ReplicatorActor;

#[test]
fn local_get_returns_response_after_update() {
  let mut actor = ReplicatorActor::<Flag, u64>::new(ReplicatorSettings::new());

  let _ = actor
    .handle_update(&Update::<Flag, ()>::new(flag_key(), WriteConsistency::Local), |_| Ok(Flag::disabled().switch_on()));

  let outcome = actor.handle_get(&Get::<Flag, ()>::new(flag_key(), ReadConsistency::Local));
  assert!(outcome.response.is_some());
}

fn flag_key() -> FlagKey {
  FlagKey::new("flag")
}
