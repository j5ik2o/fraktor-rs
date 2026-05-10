use super::{new_noop_actor_system, new_noop_actor_system_with};

#[test]
fn new_noop_actor_system_builds_default_system() {
  let system = new_noop_actor_system();

  assert_eq!(system.name(), "default-system");
}

#[test]
#[should_panic(expected = "test-support config failed to build in new_noop_actor_system_with")]
fn new_noop_actor_system_with_panics_when_config_cannot_build() {
  drop(new_noop_actor_system_with(|mut config| {
    drop(config.take_tick_driver());
    config
  }));
}
