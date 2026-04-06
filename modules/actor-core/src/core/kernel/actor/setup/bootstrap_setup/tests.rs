use core::time::Duration;

use crate::core::kernel::{
  actor::{actor_path::GuardianKind as PathGuardianKind, setup::BootstrapSetup},
  system::remote::RemotingConfig,
};

#[test]
fn bootstrap_setup_applies_bootstrap_fields() {
  let remoting = RemotingConfig::default().with_canonical_host("127.0.0.1").with_canonical_port(2552);
  let setup = BootstrapSetup::default()
    .with_system_name("bootstrap-system")
    .with_default_guardian(PathGuardianKind::System)
    .with_remoting_config(remoting)
    .with_start_time(Duration::from_secs(12));

  let config = setup.as_actor_system_config();
  assert_eq!(config.system_name(), "bootstrap-system");
  assert_eq!(config.default_guardian(), PathGuardianKind::System);
  assert_eq!(config.remoting_config().expect("remoting").canonical_port(), Some(2552));
  assert_eq!(config.start_time(), Some(Duration::from_secs(12)));
}
