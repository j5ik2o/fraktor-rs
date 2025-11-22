use crate::core::{
  outbound_action::OutboundAction, outbound_envelope::OutboundEnvelope, outbound_event::OutboundEvent,
  outbound_pipeline::OutboundPipeline, outbound_state::OutboundState,
};

fn mk(pid: &str, payload: &[u8]) -> OutboundEnvelope {
  OutboundEnvelope::new(pid.to_string(), payload.to_vec())
}

#[test]
fn disconnected_buffers_and_drops_oldest_on_overflow() {
  let mut pipeline = OutboundPipeline::new("n1:4050".to_string(), 2);
  pipeline.set_disconnected();

  assert_eq!(pipeline.state(), &OutboundState::Disconnected);

  let action1 = pipeline.send(mk("pid-1", b"m1"));
  assert_eq!(action1, OutboundAction::Enqueued { queue_len: 1 });

  let action2 = pipeline.send(mk("pid-1", b"m2"));
  assert_eq!(action2, OutboundAction::Enqueued { queue_len: 2 });

  let action3 = pipeline.send(mk("pid-1", b"m3"));
  match action3 {
    | OutboundAction::DroppedOldest { dropped, queue_len } => {
      assert_eq!(dropped.payload, b"m1".to_vec());
      assert_eq!(queue_len, 2);
    },
    | other => panic!("unexpected action: {other:?}"),
  }

  let flushed = pipeline.set_connected();
  assert_eq!(flushed, vec![mk("pid-1", b"m2"), mk("pid-1", b"m3")]);
  assert_eq!(pipeline.state(), &OutboundState::Connected);

  let events = pipeline.drain_events();
  assert_eq!(events, vec![
    OutboundEvent::Enqueued { pid: "pid-1".to_string(), queue_len: 1 },
    OutboundEvent::Enqueued { pid: "pid-1".to_string(), queue_len: 2 },
    OutboundEvent::DroppedOldest { dropped: mk("pid-1", b"m1"), reason: "queue overflow".to_string() },
    OutboundEvent::Flushed { delivered: 2 },
  ],);
}

#[test]
fn connected_dispatch_keeps_order_per_pid() {
  let mut pipeline = OutboundPipeline::new("n1:4050".to_string(), 2);
  pipeline.set_connected();

  let first = pipeline.send(mk("pid-1", b"a"));
  let second = pipeline.send(mk("pid-1", b"b"));

  assert!(matches!(first, OutboundAction::Immediate { .. }));
  assert!(matches!(second, OutboundAction::Immediate { .. }));

  let events = pipeline.drain_events();
  assert_eq!(events, vec![OutboundEvent::Dispatched { pid: "pid-1".to_string() }, OutboundEvent::Dispatched {
    pid: "pid-1".to_string(),
  },],);
}

#[test]
fn quarantine_blocks_send_and_lifts_after_deadline() {
  let mut pipeline = OutboundPipeline::new("n1:4050".to_string(), 2);
  pipeline.set_quarantine("invalid association".to_string(), Some(30));

  assert_eq!(pipeline.state(), &OutboundState::Quarantine {
    reason:   "invalid association".to_string(),
    deadline: Some(30),
  },);

  let action = pipeline.send(mk("pid-1", b"q1"));
  assert_eq!(action, OutboundAction::RejectedQuarantine { reason: "invalid association".to_string() });

  assert!(!pipeline.poll_quarantine_expiration(29));
  assert_eq!(pipeline.state(), &OutboundState::Quarantine {
    reason:   "invalid association".to_string(),
    deadline: Some(30),
  },);

  assert!(pipeline.poll_quarantine_expiration(30));
  assert_eq!(pipeline.state(), &OutboundState::Disconnected);

  let enqueued = pipeline.send(mk("pid-1", b"after"));
  assert_eq!(enqueued, OutboundAction::Enqueued { queue_len: 1 });

  let flushed = pipeline.set_connected();
  assert_eq!(flushed, vec![mk("pid-1", b"after")]);

  let events = pipeline.drain_events();
  assert_eq!(events, vec![
    OutboundEvent::Quarantined {
      authority: "n1:4050".to_string(),
      reason:    "invalid association".to_string(),
      deadline:  Some(30),
    },
    OutboundEvent::BlockedByQuarantine { pid: "pid-1".to_string(), reason: "invalid association".to_string() },
    OutboundEvent::QuarantineLifted { authority: "n1:4050".to_string() },
    OutboundEvent::Enqueued { pid: "pid-1".to_string(), queue_len: 1 },
    OutboundEvent::Flushed { delivered: 1 },
  ],);
}

#[test]
fn serialization_failure_is_recorded() {
  let mut pipeline = OutboundPipeline::new("n1:4050".to_string(), 1);

  pipeline.record_serialization_failure("pid-1".to_string(), "encode fail".to_string());

  let events = pipeline.drain_events();
  assert_eq!(events, vec![OutboundEvent::SerializationFailed {
    pid:    "pid-1".to_string(),
    reason: "encode fail".to_string(),
  }],);
}
