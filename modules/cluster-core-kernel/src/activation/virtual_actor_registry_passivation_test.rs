use core::time::Duration;

use crate::{activation::VirtualActorRegistry, extension::PassivationStrategy, grain::GrainKey};

fn key(v: &str) -> GrainKey {
  GrainKey::new(v.to_string())
}

#[test]
fn passivate_by_strategy_idle_passivates_stale_activations() {
  let mut registry = VirtualActorRegistry::new(4, 60);
  registry.ensure_activation(&key("user:1"), &["node:4000".to_string()], 0, false, None).expect("activation");

  registry
    .passivate_by_strategy(&PassivationStrategy::Idle { timeout: Duration::from_secs(10), check_interval: None }, 11);

  assert!(registry.active_keys().is_empty());
}

#[test]
fn passivate_by_strategy_active_limit_evicts_oldest() {
  let mut registry = VirtualActorRegistry::new(8, 60);
  let authorities = vec!["node:4000".to_string()];
  registry.ensure_activation(&key("user:1"), &authorities, 1, false, None).expect("activation");
  registry.ensure_activation(&key("user:2"), &authorities, 2, false, None).expect("activation");

  registry
    .passivate_by_strategy(&PassivationStrategy::ActiveLimit { limit: 1, idle_timeout: None, check_interval: None }, 3);

  assert_eq!(registry.active_keys().len(), 1);
  assert_eq!(registry.active_keys()[0].value(), "user:2");
}

#[test]
fn passivate_by_strategy_mru_evicts_most_recent() {
  let mut registry = VirtualActorRegistry::new(8, 60);
  let authorities = vec!["node:4000".to_string()];
  registry.ensure_activation(&key("user:1"), &authorities, 1, false, None).expect("activation");
  registry.ensure_activation(&key("user:2"), &authorities, 2, false, None).expect("activation");

  registry.passivate_by_strategy(&PassivationStrategy::Mru { limit: 1, idle_timeout: None, check_interval: None }, 3);

  assert_eq!(registry.active_keys().len(), 1);
  assert_eq!(registry.active_keys()[0].value(), "user:1");
}

#[test]
fn passivate_by_strategy_lfu_evicts_least_frequent() {
  let mut registry = VirtualActorRegistry::new(8, 60);
  let authorities = vec!["node:4000".to_string()];
  registry.ensure_activation(&key("user:1"), &authorities, 1, false, None).expect("activation");
  registry.ensure_activation(&key("user:2"), &authorities, 2, false, None).expect("activation");
  registry.ensure_activation(&key("user:1"), &authorities, 3, false, None).expect("activation");

  registry.passivate_by_strategy(
    &PassivationStrategy::Lfu { limit: 1, dynamic_aging: false, idle_timeout: None, check_interval: None },
    4,
  );

  assert_eq!(registry.active_keys().len(), 1);
  assert_eq!(registry.active_keys()[0].value(), "user:1");
}
