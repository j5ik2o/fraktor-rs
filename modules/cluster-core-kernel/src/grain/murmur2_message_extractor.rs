//! Kafka-compatible Murmur2 standard extractor for enveloped messages.

use alloc::string::{String, ToString};
use core::marker::PhantomData;

use super::{ShardingEnvelope, ShardingExtractorConfigError, ShardingMessageExtractor};

#[cfg(test)]
#[path = "murmur2_message_extractor_test.rs"]
mod tests;

/// Murmur2 hash compatible with Kafka's `Utils.murmur2`.
///
/// Fixed specification: seed `0x9747B28C`, mixing constant `0x5BD1E995`,
/// right shift `24`, applied to the UTF-8 bytes of the input. Arithmetic is
/// performed in the 32bit wrapping domain, matching the Java reference
/// (signed overflow in Java equals wrapping in u32; `>>>` equals logical
/// shift on u32).
fn murmur2(data: &[u8]) -> u32 {
  let length = data.len();
  let seed: u32 = 0x9747_B28C;
  let m: u32 = 0x5BD1_E995;
  let r: u32 = 24;
  let mut h: u32 = seed ^ (length as u32);
  let length4 = length / 4;
  for i in 0..length4 {
    let i4 = i * 4;
    let mut k = u32::from(data[i4])
      | (u32::from(data[i4 + 1]) << 8)
      | (u32::from(data[i4 + 2]) << 16)
      | (u32::from(data[i4 + 3]) << 24);
    k = k.wrapping_mul(m);
    k ^= k >> r;
    k = k.wrapping_mul(m);
    h = h.wrapping_mul(m);
    h ^= k;
  }
  let base = length & !3;
  let rem = length % 4;
  if rem == 3 {
    h ^= u32::from(data[base + 2]) << 16;
  }
  if rem >= 2 {
    h ^= u32::from(data[base + 1]) << 8;
  }
  if rem >= 1 {
    h ^= u32::from(data[base]);
    h = h.wrapping_mul(m);
  }
  h ^= h >> 13;
  h = h.wrapping_mul(m);
  h ^= h >> 15;
  h
}

/// Clears the sign bit, matching Kafka's `Utils.toPositive`.
const fn to_positive(hash: u32) -> u32 {
  hash & 0x7FFF_FFFF
}

/// Standard extractor deriving the shard id with Kafka-compatible Murmur2.
///
/// Mirrors Pekko's `Murmur2MessageExtractor[M]`. The shard id is
/// `(toPositive(murmur2(utf8_bytes(entity_id))) % number_of_shards)` rendered
/// as a decimal string — the same rule as Kafka's `DefaultPartitioner`, so
/// Kafka partitions can be mapped to shards. The derivation is pure and
/// identical across hosts and node topologies.
#[derive(Debug, Clone)]
pub struct Murmur2MessageExtractor<M> {
  number_of_shards: u32,
  _marker:          PhantomData<fn() -> M>,
}

impl<M> Murmur2MessageExtractor<M> {
  /// Creates a new extractor for the given number of shards.
  ///
  /// # Errors
  ///
  /// Returns [`ShardingExtractorConfigError::ShardCountZero`] if
  /// `number_of_shards` is zero.
  pub fn new(number_of_shards: u32) -> Result<Self, ShardingExtractorConfigError> {
    if number_of_shards == 0 {
      return Err(ShardingExtractorConfigError::ShardCountZero);
    }
    Ok(Self { number_of_shards, _marker: PhantomData })
  }
}

impl<M> ShardingMessageExtractor<ShardingEnvelope<M>, M> for Murmur2MessageExtractor<M> {
  fn entity_id(&self, message: &ShardingEnvelope<M>) -> Option<String> {
    Some(message.entity_id().to_string())
  }

  fn shard_id(&self, entity_id: &str) -> String {
    (to_positive(murmur2(entity_id.as_bytes())) % self.number_of_shards).to_string()
  }

  fn unwrap_message(&self, message: ShardingEnvelope<M>) -> M {
    message.into_message()
  }
}
