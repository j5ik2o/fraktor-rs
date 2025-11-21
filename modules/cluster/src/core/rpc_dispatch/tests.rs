use crate::core::{grain_key::GrainKey, rpc_dispatch::RpcDispatch, serialized_message::SerializedMessage};

#[test]
fn immediate_contains_deadline() {
  let key = GrainKey::new("k".to_string());
  let msg = SerializedMessage::new(vec![1], 1);
  let dispatch = RpcDispatch::Immediate { key, message: msg, deadline: 10 };
  assert!(matches!(dispatch, RpcDispatch::Immediate { deadline: 10, .. }));
}
