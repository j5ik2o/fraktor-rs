use super::ShardingExtractorConfigError;

#[test]
fn display_shard_count_zero_contains_cause() {
  let err = ShardingExtractorConfigError::ShardCountZero;
  let msg = alloc::format!("{err}");
  assert!(msg.contains("number of shards"), "Display should mention 'number of shards', got: {msg}");
}
