use core::time::Duration;

use crate::{
  actor_prim::actor_path::GuardianKind as PathGuardianKind,
  config::actor_system_config::{ActorSystemConfig, RemotingConfig},
};

#[test]
fn test_actor_system_config_default() {
  let config = ActorSystemConfig::default();
  assert_eq!(config.system_name(), "default-system");
  assert_eq!(config.default_guardian(), PathGuardianKind::User);
  assert!(config.remoting().is_none());
}

#[test]
fn test_actor_system_config_with_system_name() {
  let config = ActorSystemConfig::default().with_system_name("test-system");
  assert_eq!(config.system_name(), "test-system");
}

#[test]
fn test_actor_system_config_with_default_guardian() {
  let config = ActorSystemConfig::default().with_default_guardian(PathGuardianKind::System);
  assert_eq!(config.default_guardian(), PathGuardianKind::System);
}

#[test]
fn test_actor_system_config_with_remoting() {
  let remoting = RemotingConfig::default().with_canonical_host("localhost").with_canonical_port(2552);

  let config = ActorSystemConfig::default().with_remoting(remoting);

  assert!(config.remoting().is_some());
  let remoting_cfg = config.remoting().unwrap();
  assert_eq!(remoting_cfg.canonical_host(), "localhost");
  assert_eq!(remoting_cfg.canonical_port(), Some(2552));
}

#[test]
fn test_remoting_config_quarantine_duration() {
  let custom_duration = Duration::from_secs(1800);
  let remoting = RemotingConfig::default().with_quarantine_duration(custom_duration);

  assert_eq!(remoting.quarantine_duration(), custom_duration);
}

#[test]
fn test_remoting_config_defaults() {
  let remoting = RemotingConfig::default();

  // デフォルト値の検証
  assert_eq!(remoting.canonical_host(), "localhost");
  assert_eq!(remoting.canonical_port(), None);
  assert_eq!(remoting.quarantine_duration(), Duration::from_secs(5 * 24 * 3600)); // 5日
}

#[test]
#[should_panic(expected = "quarantine duration must be >= 1 second")]
fn test_remoting_config_rejects_short_quarantine() {
  let _ = RemotingConfig::default().with_quarantine_duration(Duration::from_millis(999));
}
