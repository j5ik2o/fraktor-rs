//! Hash-based standard extractor for messages without an envelope.

use alloc::{boxed::Box, string::String};
use core::fmt::{self, Formatter, Result as FmtResult};

use super::{
  ShardingExtractorConfigError, ShardingMessageExtractor, hash_code_message_extractor::pekko_hash_code_shard_id,
};

#[cfg(test)]
#[path = "hash_code_no_envelope_message_extractor_test.rs"]
mod tests;

/// User-defined entity id derivation rule.
type ExtractEntityIdFn<M> = Box<dyn Fn(&M) -> Option<String> + Send + Sync>;

/// Standard extractor applying a user-defined entity id rule to bare messages.
///
/// Mirrors Pekko's `HashCodeNoEnvelopeMessageExtractor[M]`. The entity id is
/// derived by the function given at construction (returning `None` when it
/// cannot be derived), and the shard id uses the same Pekko-compatible
/// `String.hashCode` rule as [`HashCodeMessageExtractor`](super::HashCodeMessageExtractor).
/// `unwrap_message` is the identity.
pub struct HashCodeNoEnvelopeMessageExtractor<M> {
  number_of_shards:  u32,
  extract_entity_id: ExtractEntityIdFn<M>,
}

impl<M> HashCodeNoEnvelopeMessageExtractor<M> {
  /// Creates a new extractor for the given number of shards and entity id rule.
  ///
  /// # Errors
  ///
  /// Returns [`ShardingExtractorConfigError::ShardCountZero`] if
  /// `number_of_shards` is zero.
  pub fn new(
    number_of_shards: u32,
    extract_entity_id: ExtractEntityIdFn<M>,
  ) -> Result<Self, ShardingExtractorConfigError> {
    if number_of_shards == 0 {
      return Err(ShardingExtractorConfigError::ShardCountZero);
    }
    Ok(Self { number_of_shards, extract_entity_id })
  }
}

impl<M> fmt::Debug for HashCodeNoEnvelopeMessageExtractor<M> {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    // 利用者定義の導出関数は Debug 不能なため shard 数のみを出力する
    f.debug_struct("HashCodeNoEnvelopeMessageExtractor")
      .field("number_of_shards", &self.number_of_shards)
      .finish_non_exhaustive()
  }
}

impl<M> ShardingMessageExtractor<M, M> for HashCodeNoEnvelopeMessageExtractor<M> {
  fn entity_id(&self, message: &M) -> Option<String> {
    (self.extract_entity_id)(message)
  }

  fn shard_id(&self, entity_id: &str) -> String {
    pekko_hash_code_shard_id(entity_id, self.number_of_shards)
  }

  fn unwrap_message(&self, message: M) -> M {
    message
  }
}
