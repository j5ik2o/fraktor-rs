use alloc::string::String;

use super::ShardingEnvelope;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CounterCommand {
  delta: i64,
}

#[test]
fn envelope_returns_constructed_entity_id_and_message() {
  let envelope = ShardingEnvelope::new("counter-1", CounterCommand { delta: 3 });

  assert_eq!(envelope.entity_id(), "counter-1");
  assert_eq!(envelope.message(), &CounterCommand { delta: 3 });
}

#[test]
fn envelope_accepts_owned_entity_id() {
  let entity_id = String::from("counter-2");
  let envelope = ShardingEnvelope::new(entity_id, CounterCommand { delta: -1 });

  assert_eq!(envelope.entity_id(), "counter-2");
}

#[test]
fn into_message_returns_inner_message() {
  let envelope = ShardingEnvelope::new("counter-3", CounterCommand { delta: 7 });

  let message = envelope.into_message();

  assert_eq!(message, CounterCommand { delta: 7 });
}
