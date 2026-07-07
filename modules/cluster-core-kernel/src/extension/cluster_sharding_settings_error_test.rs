use super::ClusterShardingSettingsError;

#[test]
fn display_zero_number_of_shards_contains_cause() {
  let error = ClusterShardingSettingsError::ZeroNumberOfShards;
  assert!(error.to_string().contains("number of shards"));
}

#[test]
fn display_passivation_with_remember_entities_contains_cause() {
  let error = ClusterShardingSettingsError::PassivationWithRememberEntities;
  assert!(error.to_string().contains("remember entities"));
}
