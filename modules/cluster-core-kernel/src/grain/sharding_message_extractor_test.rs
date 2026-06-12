use alloc::string::{String, ToString};

use super::ShardingMessageExtractor;

enum DeviceEvent {
  Keyed { device_id: String, payload: u32 },
  Unkeyed { payload: u32 },
}

/// 利用者定義のテストローカル extractor（要件 2.4 の受け入れ確認用）。
struct DeviceEventExtractor {
  number_of_shards: u32,
}

impl ShardingMessageExtractor<DeviceEvent, u32> for DeviceEventExtractor {
  fn entity_id(&self, message: &DeviceEvent) -> Option<String> {
    match message {
      | DeviceEvent::Keyed { device_id, .. } => Some(device_id.clone()),
      | DeviceEvent::Unkeyed { .. } => None,
    }
  }

  fn shard_id(&self, entity_id: &str) -> String {
    (entity_id.len() as u32 % self.number_of_shards).to_string()
  }

  fn unwrap_message(&self, message: DeviceEvent) -> u32 {
    match message {
      | DeviceEvent::Keyed { payload, .. } | DeviceEvent::Unkeyed { payload } => payload,
    }
  }
}

/// 契約（trait）経由で3操作を呼び出すジェネリックヘルパー。
fn derive_route<E, M, X>(extractor: &X, message: E) -> (Option<String>, M)
where
  X: ShardingMessageExtractor<E, M>, {
  let entity_id = extractor.entity_id(&message);
  let inner = extractor.unwrap_message(message);
  (entity_id, inner)
}

#[test]
fn user_defined_extractor_provides_three_operations_via_contract() {
  let extractor = DeviceEventExtractor { number_of_shards: 4 };
  let message = DeviceEvent::Keyed { device_id: String::from("device-42"), payload: 7 };

  let (entity_id, inner) = derive_route(&extractor, message);

  let entity_id = entity_id.expect("entity id derivable");
  assert_eq!(entity_id, "device-42");
  assert_eq!(extractor.shard_id(&entity_id), "1");
  assert_eq!(inner, 7);
}

#[test]
fn underivable_entity_id_is_observable_as_none() {
  let extractor = DeviceEventExtractor { number_of_shards: 4 };
  let message = DeviceEvent::Unkeyed { payload: 9 };

  let (entity_id, inner) = derive_route(&extractor, message);

  assert!(entity_id.is_none());
  assert_eq!(inner, 9);
}
