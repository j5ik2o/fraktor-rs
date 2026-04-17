use super::{super::consistent_hashable::ConsistentHashable, *};
use crate::core::kernel::actor::messaging::AnyMessage;

#[test]
fn new_retains_hash_key_and_message_payload() {
  // Given: 任意の AnyMessage と hash_key
  let inner = AnyMessage::new(123_u64);
  let hash_key = 0xDEAD_BEEF_u64;

  // When: Envelope を作成
  let envelope = ConsistentHashableEnvelope::new(inner, hash_key);

  // Then: hash_key / message の両方が取り出せる
  assert_eq!(envelope.hash_key(), hash_key);
  assert_eq!(envelope.message().downcast_ref::<u64>(), Some(&123_u64));
}

#[test]
fn consistent_hash_key_returns_stored_key() {
  // Given: Envelope
  let envelope = ConsistentHashableEnvelope::new(AnyMessage::new(0_u8), 42_u64);

  // When: ConsistentHashable トレイト経由で key を取得
  let key = <ConsistentHashableEnvelope as ConsistentHashable>::consistent_hash_key(&envelope);

  // Then: コンストラクタで与えた値と一致
  assert_eq!(key, 42_u64);
}

#[test]
fn clone_preserves_hash_key_and_shares_payload() {
  // Given: Envelope
  let original = ConsistentHashableEnvelope::new(AnyMessage::new(7_u32), 99_u64);

  // When: Clone する
  let clone = original.clone();

  // Then: hash_key は同値、AnyMessage の payload も同値を返す（ArcShared 共有）
  assert_eq!(clone.hash_key(), original.hash_key());
  assert_eq!(clone.message().downcast_ref::<u32>(), Some(&7_u32));
  assert_eq!(original.message().downcast_ref::<u32>(), Some(&7_u32));
}

#[test]
fn message_accessor_returns_shared_reference() {
  // Given: Envelope
  let envelope = ConsistentHashableEnvelope::new(AnyMessage::new("hello"), 1_u64);

  // When: message() を複数回呼ぶ
  let first = envelope.message();
  let second = envelope.message();

  // Then: いずれも同じ payload を指している
  assert_eq!(first.downcast_ref::<&'static str>(), Some(&"hello"));
  assert_eq!(second.downcast_ref::<&'static str>(), Some(&"hello"));
}
