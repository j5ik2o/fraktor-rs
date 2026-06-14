use alloc::{
  boxed::Box,
  string::{String, ToString},
};

use super::HashCodeNoEnvelopeMessageExtractor;
use crate::grain::{HashCodeMessageExtractor, ShardingExtractorConfigError, ShardingMessageExtractor};

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountCommand {
  account_id: Option<String>,
  amount:     i64,
}

fn account_extractor(number_of_shards: u32) -> HashCodeNoEnvelopeMessageExtractor<AccountCommand> {
  HashCodeNoEnvelopeMessageExtractor::new(number_of_shards, Box::new(|m: &AccountCommand| m.account_id.clone()))
    .expect("extractor")
}

#[test]
fn new_rejects_zero_shard_count() {
  let result =
    HashCodeNoEnvelopeMessageExtractor::<AccountCommand>::new(0, Box::new(|m: &AccountCommand| m.account_id.clone()));

  assert_eq!(result.unwrap_err(), ShardingExtractorConfigError::ShardCountZero);
}

#[test]
fn entity_id_applies_user_defined_rule() {
  let extractor = account_extractor(4);
  let message = AccountCommand { account_id: Some(String::from("acct-1")), amount: 100 };

  assert_eq!(extractor.entity_id(&message), Some(String::from("acct-1")));
}

#[test]
fn underivable_entity_id_propagates_as_none() {
  let extractor = account_extractor(4);
  let message = AccountCommand { account_id: None, amount: 100 };

  assert_eq!(extractor.entity_id(&message), None);
}

#[test]
fn unwrap_message_is_identity() {
  let extractor = account_extractor(4);
  let message = AccountCommand { account_id: Some(String::from("acct-1")), amount: 100 };

  assert_eq!(extractor.unwrap_message(message.clone()), message);
}

#[test]
fn shard_id_matches_hash_code_extractor_for_same_entity_id() {
  // HashCode 標準実装（共有 Pekko hashCode）と同一の shard 規則であることを実装間比較で検証する。
  let no_envelope = account_extractor(10);
  let with_envelope = HashCodeMessageExtractor::<u32>::new(10).expect("extractor");

  for entity_id in ["counter-1", "device-42", "acct-1"] {
    assert_eq!(no_envelope.shard_id(entity_id), with_envelope.shard_id(entity_id), "entity_id={entity_id}");
  }
  // 既知ベクタによる固定（"counter-1".hashCode = 1352256672, % 10 = 2）
  assert_eq!(no_envelope.shard_id("counter-1"), "2");
}

#[test]
fn shard_id_is_deterministic_across_instances() {
  let first = account_extractor(16);
  let second = HashCodeNoEnvelopeMessageExtractor::new(
    16,
    Box::new(|m: &AccountCommand| m.account_id.as_ref().map(|id| id.to_string())),
  )
  .expect("extractor");

  assert_eq!(first.shard_id("acct-1"), second.shard_id("acct-1"));
}
