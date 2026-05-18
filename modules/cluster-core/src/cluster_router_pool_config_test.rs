use crate::cluster_router_pool_config::ClusterRouterPoolConfig;

#[test]
fn pool_settings_store_values() {
  let settings = ClusterRouterPoolConfig::new(5).with_allow_local_routees(false);
  assert_eq!(settings.total_instances(), 5);
  assert!(!settings.allow_local_routees());
}

#[test]
#[should_panic(expected = "total instances must be > 0")]
fn pool_settings_reject_zero_instances() {
  let _ = ClusterRouterPoolConfig::new(0);
}
