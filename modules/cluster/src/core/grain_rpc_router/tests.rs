use crate::core::{
  dispatch_drop_policy::DispatchDropPolicy,
  grain_key::GrainKey,
  grain_rpc_router::GrainRpcRouter,
  rpc_dispatch::RpcDispatch,
  rpc_error::RpcError,
  rpc_event::RpcEvent,
  serialized_message::SerializedMessage,
};

fn key(v: &str) -> GrainKey {
  GrainKey::new(v.to_string())
}

fn msg(ver: u32, payload: &[u8]) -> SerializedMessage {
  SerializedMessage::new(payload.to_vec(), ver)
}

#[test]
fn negotiates_and_dispatches_immediate() {
  let mut router = GrainRpcRouter::new(1, 1, DispatchDropPolicy::DropOldest, vec![1, 2]);
  assert_eq!(router.negotiate(&[2]), Some(2));

  let dispatch = router.dispatch(key("k"), msg(2, b"hi"), 10).expect("dispatch");
  assert!(matches!(dispatch, RpcDispatch::Immediate { .. }));

  let events = router.drain_events();
  assert!(events.iter().any(|e| matches!(e, RpcEvent::Dispatched { .. })));
}

#[test]
fn schema_mismatch_is_error() {
  let mut router = GrainRpcRouter::new(1, 1, DispatchDropPolicy::DropOldest, vec![2]);
  router.negotiate(&[2]);
  let err = router.dispatch(key("k"), msg(1, b"hi"), 10).expect_err("mismatch");
  assert_eq!(err, RpcError::SchemaMismatch { negotiated: Some(2), message_version: 1 });
}

#[test]
fn empty_payload_is_serialization_error() {
  let mut router = GrainRpcRouter::new(1, 1, DispatchDropPolicy::DropOldest, vec![1]);
  router.negotiate(&[1]);
  let err = router.dispatch(key("k"), msg(1, b""), 5).expect_err("empty");
  assert!(matches!(err, RpcError::SerializationFailed { .. }));
  let events = router.drain_events();
  assert!(events.iter().any(|e| matches!(e, RpcEvent::SerializationFailed { .. })));
}

#[test]
fn concurrency_limit_promotes_queued_and_drops_oldest() {
  let mut router = GrainRpcRouter::new(1, 1, DispatchDropPolicy::DropOldest, vec![1]);
  router.negotiate(&[1]);

  let first = router.dispatch(key("k"), msg(1, b"a"), 5).expect("ok");
  assert!(matches!(first, RpcDispatch::Immediate { .. }));

  let second = router.dispatch(key("k"), msg(1, b"b"), 6).expect("queued");
  assert!(matches!(second, RpcDispatch::Queued { queue_len: 1 }));

  let third = router.dispatch(key("k"), msg(1, b"c"), 7).expect("drop oldest");
  assert!(matches!(third, RpcDispatch::Queued { .. }));

  let promoted = router.complete(&key("k"), 2).expect("promote");
  assert!(matches!(promoted, RpcDispatch::Immediate { .. }));

  let events = router.drain_events();
  assert!(events.iter().any(|e| matches!(e, RpcEvent::Queued { .. })));
  assert!(events.iter().any(|e| matches!(e, RpcEvent::DroppedOldest { .. })));
  assert!(events.iter().any(|e| matches!(e, RpcEvent::Promoted { .. })));
}

#[test]
fn timeout_on_promoted_request_is_reported() {
  let mut router = GrainRpcRouter::new(1, 1, DispatchDropPolicy::RejectNew, vec![1]);
  router.negotiate(&[1]);
  router.dispatch(key("k"), msg(1, b"a"), 1).expect("first");
  router.dispatch(key("k"), msg(1, b"b"), 2).expect("queued");

  let dropped = router.complete(&key("k"), 3).expect("timeout");
  assert!(matches!(dropped, RpcDispatch::Dropped { .. }));
  let events = router.drain_events();
  assert!(events.iter().any(|e| matches!(e, RpcEvent::TimedOut { .. })));
}
