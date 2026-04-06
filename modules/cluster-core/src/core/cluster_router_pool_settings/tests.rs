use crate::core::cluster_router_pool_settings::ClusterRouterPoolSettings;

#[test]
fn pool_settings_store_values() {
  let settings = ClusterRouterPoolSettings::new(5).with_allow_local_routees(false);
  assert_eq!(settings.total_instances(), 5);
  assert!(!settings.allow_local_routees());
}

#[test]
#[should_panic(expected = "total instances must be > 0")]
fn pool_settings_reject_zero_instances() {
  let _ = ClusterRouterPoolSettings::new(0);
}
