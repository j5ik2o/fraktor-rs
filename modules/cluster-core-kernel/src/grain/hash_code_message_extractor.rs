//! Hash-based standard extractor for enveloped messages.

use alloc::string::{String, ToString};
use core::marker::PhantomData;

use super::{ShardingEnvelope, ShardingExtractorConfigError, ShardingMessageExtractor};

#[cfg(test)]
#[path = "hash_code_message_extractor_test.rs"]
mod tests;

/// JVM `String.hashCode` compatible hash shared by the hash-code standard extractors.
///
/// The hash is computed over UTF-16 code units with wrapping 32bit signed
/// arithmetic, matching Java and Scala `String.hashCode`.
pub(super) fn pekko_hash_code(value: &str) -> i32 {
  let mut hash = 0_i32;
  for code_unit in value.encode_utf16() {
    hash = hash.wrapping_mul(31).wrapping_add(i32::from(code_unit));
  }
  hash
}

/// Pekko-compatible shard id for the given entity id and shard count.
pub(super) fn pekko_hash_code_shard_id(entity_id: &str, number_of_shards: u32) -> String {
  let hash = pekko_hash_code(entity_id);
  let positive_hash = if hash == i32::MIN { i32::MIN } else { hash.abs() };
  let shard = i64::from(positive_hash) % i64::from(number_of_shards);
  shard.to_string()
}

/// Standard extractor deriving the shard id from the envelope entity id.
///
/// Mirrors Pekko's `HashCodeMessageExtractor[M]`. The shard id is derived by
/// `math.abs(entityId.hashCode) % numberOfShards`, rendered as a decimal
/// string, including the JVM `Int.MinValue` edge case.
#[derive(Debug, Clone)]
pub struct HashCodeMessageExtractor<M> {
  number_of_shards: u32,
  _marker:          PhantomData<fn() -> M>,
}

impl<M> HashCodeMessageExtractor<M> {
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

impl<M> ShardingMessageExtractor<ShardingEnvelope<M>, M> for HashCodeMessageExtractor<M> {
  fn entity_id(&self, message: &ShardingEnvelope<M>) -> Option<String> {
    Some(message.entity_id().to_string())
  }

  fn shard_id(&self, entity_id: &str) -> String {
    pekko_hash_code_shard_id(entity_id, self.number_of_shards)
  }

  fn unwrap_message(&self, message: ShardingEnvelope<M>) -> M {
    message.into_message()
  }
}
