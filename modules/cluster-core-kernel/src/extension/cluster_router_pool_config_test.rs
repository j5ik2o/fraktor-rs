use crate::extension::ClusterRouterPoolConfig;

#[test]
fn pool_settings_store_values() {
  let settings = ClusterRouterPoolConfig::new(5)
    .with_allow_local_routees(false)
    .with_use_roles(vec![String::from("worker"), String::from("backend"), String::from("backend")])
    .with_max_instances_per_node(2);
  assert_eq!(settings.total_instances(), 5);
  assert!(!settings.allow_local_routees());
  assert_eq!(settings.use_roles(), &[String::from("backend"), String::from("worker")]);
  assert_eq!(settings.max_instances_per_node(), Some(2));
}

#[test]
#[should_panic(expected = "total instances must be > 0")]
fn pool_settings_reject_zero_instances() {
  drop(ClusterRouterPoolConfig::new(0));
}

#[test]
#[should_panic(expected = "max instances per node must be > 0")]
fn pool_settings_reject_zero_max_instances_per_node() {
  drop(ClusterRouterPoolConfig::new(1).with_max_instances_per_node(0));
}
