use super::ClusterShardingStateStoreMode;

#[test]
fn default_is_in_memory() {
  assert_eq!(ClusterShardingStateStoreMode::default(), ClusterShardingStateStoreMode::InMemory);
}

#[test]
fn as_str_returns_stable_configuration_value() {
  assert_eq!(ClusterShardingStateStoreMode::InMemory.as_str(), "in-memory");
  assert_eq!(ClusterShardingStateStoreMode::Durable.as_str(), "durable");
}
