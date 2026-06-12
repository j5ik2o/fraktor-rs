use alloc::string::String;

use super::Murmur2MessageExtractor;
use crate::grain::{ShardingEnvelope, ShardingExtractorConfigError, ShardingMessageExtractor};

#[test]
fn new_rejects_zero_shard_count() {
  let result = Murmur2MessageExtractor::<u32>::new(0);

  assert_eq!(result.unwrap_err(), ShardingExtractorConfigError::ShardCountZero);
}

#[test]
fn entity_id_is_taken_from_envelope() {
  let extractor = Murmur2MessageExtractor::<u32>::new(4).expect("extractor");
  let envelope = ShardingEnvelope::new("order-7", 7u32);

  assert_eq!(extractor.entity_id(&envelope), Some(String::from("order-7")));
}

#[test]
fn unwrap_message_returns_inner_message() {
  let extractor = Murmur2MessageExtractor::<u32>::new(4).expect("extractor");
  let envelope = ShardingEnvelope::new("order-7", 7u32);

  assert_eq!(extractor.unwrap_message(envelope), 7);
}

#[test]
fn shard_id_is_deterministic_across_instances() {
  let first = Murmur2MessageExtractor::<u32>::new(16).expect("extractor");
  let second = Murmur2MessageExtractor::<u32>::new(16).expect("extractor");

  assert_eq!(first.shard_id("order-7"), second.shard_id("order-7"));
}

#[test]
fn shard_id_matches_kafka_reference_vectors() {
  // Kafka DefaultPartitioner 互換の参照ベクタ。
  // 出典: Kafka `Utils.murmur2`（https://github.com/apache/kafka/blob/db42afd6e24ef4291390b4d1c1f10758beedefed/
  //   clients/src/main/java/org/apache/kafka/common/utils/Utils.java#L500）の既知出力。
  //   Pekko `cluster-sharding-typed` の `Murmur2Spec.scala` にも同一ベクタが掲載されている:
  //   murmur2("1") = -1993445489, murmur2("12") = 126087238, murmur2("123") = -267702483,
  //   murmur2("1234") = -1614185708, murmur2("12345") = -1188365604
  // shard id は toPositive(h) = h & 0x7FFFFFFF を経て % n を文字列化した値:
  //   toPositive: "1" → 154038159, "12" → 126087238, "123" → 1879781165,
  //   "1234" → 533297940, "12345" → 959118044
  let ten_shards = Murmur2MessageExtractor::<u32>::new(10).expect("extractor");
  assert_eq!(ten_shards.shard_id("1"), "9");
  assert_eq!(ten_shards.shard_id("12"), "8");
  assert_eq!(ten_shards.shard_id("123"), "5");
  assert_eq!(ten_shards.shard_id("1234"), "0");
  assert_eq!(ten_shards.shard_id("12345"), "4");

  let hundred_shards = Murmur2MessageExtractor::<u32>::new(100).expect("extractor");
  assert_eq!(hundred_shards.shard_id("1"), "59");
  assert_eq!(hundred_shards.shard_id("12"), "38");
  assert_eq!(hundred_shards.shard_id("123"), "65");
  assert_eq!(hundred_shards.shard_id("1234"), "40");
  assert_eq!(hundred_shards.shard_id("12345"), "44");

  // u32::MAX を shard 数にすると toPositive(murmur2(x)) がそのまま shard id になり、
  // ハッシュ仕様自体を固定できる（toPositive の結果は常に 0x7FFFFFFF 以下）。
  let identity_shards = Murmur2MessageExtractor::<u32>::new(u32::MAX).expect("extractor");
  assert_eq!(identity_shards.shard_id("1"), "154038159");
  assert_eq!(identity_shards.shard_id("12"), "126087238");
  assert_eq!(identity_shards.shard_id("123"), "1879781165");
  assert_eq!(identity_shards.shard_id("1234"), "533297940");
  assert_eq!(identity_shards.shard_id("12345"), "959118044");
}
