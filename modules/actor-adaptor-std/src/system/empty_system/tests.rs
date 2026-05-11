use super::{create_noop_actor_system, create_noop_actor_system_with};

#[test]
fn create_noop_actor_system_builds_default_system() {
  let system = create_noop_actor_system();

  assert_eq!(system.name(), "default-system");
}

#[test]
#[should_panic(expected = "test-support config failed to build in create_noop_actor_system_with")]
fn create_noop_actor_system_with_panics_when_config_cannot_build() {
  drop(create_noop_actor_system_with(|mut config| {
    let _ = config.take_tick_driver();
    config
  }));
}
