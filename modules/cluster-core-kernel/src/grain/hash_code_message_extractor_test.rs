use alloc::string::String;

use super::HashCodeMessageExtractor;
use crate::grain::{ShardingEnvelope, ShardingExtractorConfigError, ShardingMessageExtractor};

#[test]
fn new_rejects_zero_shard_count() {
  let result = HashCodeMessageExtractor::<u32>::new(0);

  assert_eq!(result.unwrap_err(), ShardingExtractorConfigError::ShardCountZero);
}

#[test]
fn entity_id_is_taken_from_envelope() {
  let extractor = HashCodeMessageExtractor::<u32>::new(4).expect("extractor");
  let envelope = ShardingEnvelope::new("counter-1", 7u32);

  assert_eq!(extractor.entity_id(&envelope), Some(String::from("counter-1")));
}

#[test]
fn unwrap_message_returns_inner_message() {
  let extractor = HashCodeMessageExtractor::<u32>::new(4).expect("extractor");
  let envelope = ShardingEnvelope::new("counter-1", 7u32);

  assert_eq!(extractor.unwrap_message(envelope), 7);
}

#[test]
fn shard_id_is_deterministic_for_same_input() {
  let extractor = HashCodeMessageExtractor::<u32>::new(16).expect("extractor");

  assert_eq!(extractor.shard_id("counter-1"), extractor.shard_id("counter-1"));
}

#[test]
fn shard_id_matches_known_pekko_hash_code_vectors() {
  // Pekko は Scala/JVM の String.hashCode を使うため、UTF-16 code unit
  // による 31 倍加算と math.abs(hash) % numberOfShards の結果を固定する。
  let identity_shards = HashCodeMessageExtractor::<u32>::new(u32::MAX).expect("extractor");
  assert_eq!(identity_shards.shard_id("counter-1"), "1352256672");
  assert_eq!(identity_shards.shard_id("device-42"), "25160021");
  assert_eq!(identity_shards.shard_id("acct-1"), "1423448841");
  assert_eq!(identity_shards.shard_id(""), "0");

  let ten_shards = HashCodeMessageExtractor::<u32>::new(10).expect("extractor");
  assert_eq!(ten_shards.shard_id("counter-1"), "2");
  assert_eq!(ten_shards.shard_id("device-42"), "1");
  assert_eq!(ten_shards.shard_id("acct-1"), "1");
  assert_eq!(ten_shards.shard_id("Aa"), "2");
  assert_eq!(ten_shards.shard_id("BB"), "2");
  assert_eq!(ten_shards.shard_id("polygenelubricants"), "-8");
}
