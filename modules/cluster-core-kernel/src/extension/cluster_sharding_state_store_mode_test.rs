use super::ClusterShardingStateStoreMode;

#[test]
fn default_is_ddata() {
  assert_eq!(ClusterShardingStateStoreMode::default(), ClusterShardingStateStoreMode::DData);
}

#[test]
fn as_str_returns_pekko_compatible_configuration_value() {
  assert_eq!(ClusterShardingStateStoreMode::DData.as_str(), "ddata");
  assert_eq!(ClusterShardingStateStoreMode::Persistence.as_str(), "persistence");
}
