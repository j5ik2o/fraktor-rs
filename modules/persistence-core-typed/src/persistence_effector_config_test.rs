use crate::{PersistenceEffectorConfig, PersistenceId};

fn apply_event(state: &u32, event: &u32) -> u32 {
  state + event
}

#[test]
fn default_stash_capacity_is_bounded() {
  let config = PersistenceEffectorConfig::<u32, u32, ()>::new(PersistenceId::of_unique_id("test"), 0, apply_event);

  assert_eq!(config.stash_capacity(), 1000);
  assert!(config.validate().is_ok());
}
