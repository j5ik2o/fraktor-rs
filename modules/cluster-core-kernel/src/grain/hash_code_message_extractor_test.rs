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
fn shard_id_matches_known_fnv1a_vectors() {
  // FNV-1a 32bit の既知ベクタでハッシュ仕様を固定する。
  // 期待値の根拠: offset basis 0x811C9DC5 / prime 0x01000193 による手計算
  // （fnv1a32("counter-1") = 0xA5D60CE1 = 2782268641, fnv1a32("device-42") = 0xE9174B68 = 3910617960,
  //   fnv1a32("") = 0x811C9DC5 = 2166136261 = offset basis）。
  // shard 数に u32::MAX を使うとハッシュ値がそのまま shard id
  // になるため、ハッシュ仕様自体を固定できる。
  let identity_shards = HashCodeMessageExtractor::<u32>::new(u32::MAX).expect("extractor");
  assert_eq!(identity_shards.shard_id("counter-1"), "2782268641");
  assert_eq!(identity_shards.shard_id("device-42"), "3910617960");
  assert_eq!(identity_shards.shard_id(""), "2166136261");

  let ten_shards = HashCodeMessageExtractor::<u32>::new(10).expect("extractor");
  assert_eq!(ten_shards.shard_id("counter-1"), "1");
  assert_eq!(ten_shards.shard_id("device-42"), "0");
}
