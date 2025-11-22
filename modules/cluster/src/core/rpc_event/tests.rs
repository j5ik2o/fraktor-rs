use crate::core::{grain_key::GrainKey, rpc_event::RpcEvent};

fn key() -> GrainKey {
  GrainKey::new("k".to_string())
}

#[test]
fn queued_event_keeps_length() {
  let ev = RpcEvent::Queued { key: key(), queue_len: 2 };
  assert!(matches!(ev, RpcEvent::Queued { queue_len: 2, .. }));
}
