use super::ActorSystem;
use crate::actor::{scheduler::tick_driver::tests::TestTickDriver, setup::ActorSystemConfig};

#[test]
fn upgrade_returns_actor_system_while_state_is_alive() {
  let system =
    ActorSystem::create_with_noop_guardian(ActorSystemConfig::new(TestTickDriver::default())).expect("system");
  let weak = system.downgrade();

  let upgraded = weak.upgrade().expect("weak system should upgrade while the original system is alive");

  assert_eq!(upgraded.name(), system.name());
}
