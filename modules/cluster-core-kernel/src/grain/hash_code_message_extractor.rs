//! Hash-based standard extractor for enveloped messages.

use alloc::string::{String, ToString};
use core::marker::PhantomData;

use super::{ShardingEnvelope, ShardingExtractorConfigError, ShardingMessageExtractor};

#[cfg(test)]
#[path = "hash_code_message_extractor_test.rs"]
mod tests;

/// FNV-1a 32bit hash shared by the hash-code standard extractors.
///
/// Fixed specification: offset basis `0x811C9DC5`, prime `0x01000193`,
/// applied to the UTF-8 bytes of the input.
pub(super) fn fnv1a_32(value: &str) -> u32 {
  let mut hash: u32 = 0x811C_9DC5;
  for byte in value.as_bytes() {
    hash ^= u32::from(*byte);
    hash = hash.wrapping_mul(0x0100_0193);
  }
  hash
}

/// Standard extractor deriving the shard id from the envelope entity id.
///
/// Mirrors Pekko's `HashCodeMessageExtractor[M]`. The hash function is fixed
/// to FNV-1a 32bit (offset basis `0x811C9DC5`, prime `0x01000193`) over the
/// UTF-8 bytes of the entity id, and the shard id is
/// `(hash % number_of_shards)` rendered as a decimal string. The derivation
/// is pure and identical across hosts and node topologies.
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
    (fnv1a_32(entity_id) % self.number_of_shards).to_string()
  }

  fn unwrap_message(&self, message: ShardingEnvelope<M>) -> M {
    message.into_message()
  }
}
