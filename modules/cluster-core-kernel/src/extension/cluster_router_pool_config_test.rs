use alloc::{string::String, vec};

use crate::extension::ClusterRouterPoolConfig;

#[test]
fn pool_settings_store_values() {
  let settings = ClusterRouterPoolConfig::new(5).with_allow_local_routees(false);
  assert_eq!(settings.total_instances(), 5);
  assert!(!settings.allow_local_routees());
}

#[test]
fn pool_settings_default_to_one_instance_per_node_and_no_roles() {
  let settings = ClusterRouterPoolConfig::new(5);
  assert_eq!(settings.max_instances_per_node(), 1);
  assert!(settings.use_roles().is_empty());
}

#[test]
fn with_max_instances_per_node_overrides_value() {
  let settings = ClusterRouterPoolConfig::new(5).with_max_instances_per_node(3);
  assert_eq!(settings.max_instances_per_node(), 3);
}

#[test]
#[should_panic(expected = "max instances per node must be > 0")]
fn with_max_instances_per_node_rejects_zero() {
  drop(ClusterRouterPoolConfig::new(5).with_max_instances_per_node(0));
}

#[test]
#[should_panic(expected = "total instances must be > 0")]
fn pool_settings_reject_zero_instances() {
  drop(ClusterRouterPoolConfig::new(0));
}

#[test]
fn with_use_roles_stores_roles() {
  let settings = ClusterRouterPoolConfig::new(5).with_use_roles(vec![String::from("backend")]);
  assert_eq!(settings.use_roles(), &[String::from("backend")]);
}

#[test]
fn satisfies_roles_requires_all_configured_roles() {
  let settings = ClusterRouterPoolConfig::new(5).with_use_roles(vec![String::from("backend"), String::from("compute")]);
  // A node carrying every required role (plus extras) qualifies.
  assert!(settings.satisfies_roles(&[String::from("backend"), String::from("compute"), String::from("web")]));
  // Missing one required role disqualifies the node.
  assert!(!settings.satisfies_roles(&[String::from("backend")]));
  // A node carrying no roles never satisfies a non-empty requirement.
  assert!(!settings.satisfies_roles(&[]));
}

#[test]
fn satisfies_roles_with_empty_requirement_matches_any_node() {
  let settings = ClusterRouterPoolConfig::new(5);
  assert!(settings.satisfies_roles(&[]));
  assert!(settings.satisfies_roles(&[String::from("anything")]));
}
