use fraktor_actor_rs::core::event_stream::RemotingLifecycleEvent;

use super::{EndpointManager, EndpointManagerCommand, EndpointManagerEffect};
use crate::core::{
  association_state::AssociationState,
  deferred_envelope::DeferredEnvelope,
  quarantine_reason::QuarantineReason,
  remote_node_id::RemoteNodeId,
  transport::{LoopbackTransport, RemoteTransport, TransportBind, TransportEndpoint},
};

fn manager() -> EndpointManager {
  EndpointManager::new()
}

fn sample_endpoint() -> TransportEndpoint {
  TransportEndpoint::new("loopback:4100".into())
}

fn sample_endpoint_alt() -> TransportEndpoint {
  TransportEndpoint::new("loopback:4101".into())
}

fn sample_remote() -> RemoteNodeId {
  RemoteNodeId::new("system-b", "127.0.0.1", Some(4200), 7)
}

fn envelope(label: &str) -> DeferredEnvelope {
  DeferredEnvelope::new(label)
}

struct LoopbackPair {
  _transport:  LoopbackTransport,
  authority_a: String,
  authority_b: String,
}

impl LoopbackPair {
  fn new() -> Self {
    let transport = LoopbackTransport::default();
    let bind_a = TransportBind::new("loopback-a.local", Some(4100));
    let handle_a = transport.spawn_listener(&bind_a).expect("listener a");
    let bind_b = TransportBind::new("loopback-b.local", Some(4200));
    let handle_b = transport.spawn_listener(&bind_b).expect("listener b");
    Self {
      _transport:  transport,
      authority_a: handle_a.authority().to_string(),
      authority_b: handle_b.authority().to_string(),
    }
  }

  fn authority_for_manager_a(&self) -> String {
    self.authority_b.clone()
  }

  fn authority_for_manager_b(&self) -> String {
    self.authority_a.clone()
  }

  fn endpoint_to_manager_a(&self) -> TransportEndpoint {
    TransportEndpoint::new(self.authority_a.clone())
  }

  fn endpoint_to_manager_b(&self) -> TransportEndpoint {
    TransportEndpoint::new(self.authority_b.clone())
  }
}

#[test]
fn register_and_handshake_transitions_states() {
  let mgr = manager();
  let register = EndpointManagerCommand::RegisterInbound { authority: "loopback:4100".into(), now: 1 };
  let result = mgr.handle(register);
  assert!(result.effects.is_empty());

  let associate =
    EndpointManagerCommand::Associate { authority: "loopback:4100".into(), endpoint: sample_endpoint(), now: 2 };
  let result = mgr.handle(associate);
  assert_eq!(result.effects, vec![EndpointManagerEffect::StartHandshake {
    authority: "loopback:4100".into(),
    endpoint:  sample_endpoint(),
  }]);

  let accept = EndpointManagerCommand::HandshakeAccepted {
    authority:   "loopback:4100".into(),
    remote_node: sample_remote(),
    now:         3,
  };
  let result = mgr.handle(accept);
  assert_eq!(result.effects.len(), 1);
  match &result.effects[0] {
    | EndpointManagerEffect::Lifecycle(event) => match event {
      | RemotingLifecycleEvent::Connected { authority, remote_system, remote_uid, correlation_id } => {
        assert_eq!(authority, "loopback:4100");
        assert_eq!(remote_system, "system-b");
        assert_eq!(*remote_uid, 7);
        assert!(!correlation_id.is_nil());
      },
      | other => panic!("unexpected lifecycle event: {other:?}"),
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
  assert!(matches!(mgr.state("loopback:4100"), Some(AssociationState::Connected { .. })));
}

#[test]
fn deferred_messages_flush_on_connected() {
  let mgr = manager();
  let authority = "loopback:4200".to_string();
  mgr.handle(EndpointManagerCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointManagerCommand::Associate {
    authority: authority.clone(),
    endpoint:  sample_endpoint(),
    now:       2,
  });

  let enqueue = EndpointManagerCommand::EnqueueDeferred { authority: authority.clone(), envelope: envelope("m1") };
  let result = mgr.handle(enqueue);
  assert!(result.effects.is_empty());

  let result = mgr.handle(EndpointManagerCommand::HandshakeAccepted {
    authority:   authority.clone(),
    remote_node: sample_remote(),
    now:         3,
  });
  assert_eq!(result.effects.len(), 2);
  match &result.effects[0] {
    | EndpointManagerEffect::DeliverEnvelopes { authority: deliver_authority, envelopes } => {
      assert_eq!(deliver_authority, &authority);
      assert_eq!(envelopes, &vec![envelope("m1")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
  match &result.effects[1] {
    | EndpointManagerEffect::Lifecycle(event) => match event {
      | RemotingLifecycleEvent::Connected { authority: connected_authority, .. } => {
        assert_eq!(connected_authority, &authority);
      },
      | other => panic!("unexpected lifecycle event: {other:?}"),
    },
    | other => panic!("unexpected effect: {other:?}"),
  }

  let immediate =
    mgr.handle(EndpointManagerCommand::EnqueueDeferred { authority: authority.clone(), envelope: envelope("m2") });
  assert_eq!(immediate.effects, vec![EndpointManagerEffect::DeliverEnvelopes {
    authority,
    envelopes: vec![envelope("m2")],
  }]);
}

#[test]
fn quarantine_discards_deferred_messages() {
  let mgr = manager();
  let authority = "loopback:4300".to_string();
  mgr.handle(EndpointManagerCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointManagerCommand::Associate {
    authority: authority.clone(),
    endpoint:  sample_endpoint(),
    now:       2,
  });
  mgr.handle(EndpointManagerCommand::EnqueueDeferred { authority: authority.clone(), envelope: envelope("m1") });

  let reason = QuarantineReason::new("uid mismatch");
  let result = mgr.handle(EndpointManagerCommand::Quarantine {
    authority: authority.clone(),
    reason:    reason.clone(),
    resume_at: Some(50),
    now:       3,
  });

  assert_eq!(result.effects.len(), 2);
  match &result.effects[0] {
    | EndpointManagerEffect::DiscardDeferred { authority: discard_authority, reason: discard_reason, envelopes } => {
      assert_eq!(discard_authority, &authority);
      assert_eq!(discard_reason, &reason);
      assert_eq!(envelopes, &vec![envelope("m1")]);
    },
    | other => panic!("unexpected discard effect: {other:?}"),
  }
  match &result.effects[1] {
    | EndpointManagerEffect::Lifecycle(event) => match event {
      | RemotingLifecycleEvent::Quarantined { authority: quarantined_authority, reason: msg, correlation_id } => {
        assert_eq!(quarantined_authority, &authority);
        assert_eq!(msg, reason.message());
        assert!(!correlation_id.is_nil());
      },
      | other => panic!("unexpected lifecycle event: {other:?}"),
    },
    | other => panic!("unexpected effect: {other:?}"),
  }

  assert_eq!(mgr.state(&authority), Some(AssociationState::Quarantined { reason, resume_at: Some(50) }));
}

#[test]
fn recover_from_quarantine_restarts_handshake() {
  let mgr = manager();
  let authority = "loopback:4400".to_string();
  mgr.handle(EndpointManagerCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointManagerCommand::Associate {
    authority: authority.clone(),
    endpoint:  sample_endpoint(),
    now:       2,
  });
  mgr.handle(EndpointManagerCommand::Quarantine {
    authority: authority.clone(),
    reason:    QuarantineReason::new("network failure"),
    resume_at: None,
    now:       3,
  });

  mgr.handle(EndpointManagerCommand::EnqueueDeferred { authority: authority.clone(), envelope: envelope("m2") });

  let result = mgr.handle(EndpointManagerCommand::Recover {
    authority: authority.clone(),
    endpoint:  Some(sample_endpoint_alt()),
    now:       4,
  });
  assert_eq!(result.effects, vec![EndpointManagerEffect::StartHandshake {
    authority: authority.clone(),
    endpoint:  sample_endpoint_alt(),
  }]);

  let result = mgr.handle(EndpointManagerCommand::HandshakeAccepted {
    authority:   authority.clone(),
    remote_node: sample_remote(),
    now:         5,
  });
  assert_eq!(result.effects.len(), 2);
  match &result.effects[0] {
    | EndpointManagerEffect::DeliverEnvelopes { authority: deliver_authority, envelopes } => {
      assert_eq!(deliver_authority, &authority);
      assert_eq!(envelopes, &vec![envelope("m2")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
  match &result.effects[1] {
    | EndpointManagerEffect::Lifecycle(event) => match event {
      | RemotingLifecycleEvent::Connected { authority: connected_authority, .. } => {
        assert_eq!(connected_authority, &authority);
      },
      | other => panic!("unexpected lifecycle event: {other:?}"),
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
}

#[test]
fn loopback_pair_association_flushes_deferred_and_emits_connected_events() {
  let loopback = LoopbackPair::new();
  let manager_a = manager();
  let manager_b = manager();
  let authority_for_a = loopback.authority_for_manager_a();
  let authority_for_b = loopback.authority_for_manager_b();

  manager_a.handle(EndpointManagerCommand::RegisterInbound { authority: authority_for_a.clone(), now: 1 });
  manager_b.handle(EndpointManagerCommand::RegisterInbound { authority: authority_for_b.clone(), now: 1 });

  manager_a.handle(EndpointManagerCommand::EnqueueDeferred {
    authority: authority_for_a.clone(),
    envelope:  envelope("a->b"),
  });
  manager_b.handle(EndpointManagerCommand::EnqueueDeferred {
    authority: authority_for_b.clone(),
    envelope:  envelope("b->a"),
  });

  let handshake_a = manager_a.handle(EndpointManagerCommand::Associate {
    authority: authority_for_a.clone(),
    endpoint:  loopback.endpoint_to_manager_b(),
    now:       2,
  });
  assert!(matches!(handshake_a.effects.as_slice(), [EndpointManagerEffect::StartHandshake { .. }]));

  let handshake_b = manager_b.handle(EndpointManagerCommand::Associate {
    authority: authority_for_b.clone(),
    endpoint:  loopback.endpoint_to_manager_a(),
    now:       2,
  });
  assert!(matches!(handshake_b.effects.as_slice(), [EndpointManagerEffect::StartHandshake { .. }]));

  let node_b = RemoteNodeId::new("system-b", "loopback-b.local", Some(4200), 99);
  let result_a = manager_a.handle(EndpointManagerCommand::HandshakeAccepted {
    authority:   authority_for_a.clone(),
    remote_node: node_b,
    now:         3,
  });
  assert_eq!(result_a.effects.len(), 2);
  match &result_a.effects[0] {
    | EndpointManagerEffect::DeliverEnvelopes { envelopes, .. } => {
      assert_eq!(envelopes, &vec![envelope("a->b")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
  match &result_a.effects[1] {
    | EndpointManagerEffect::Lifecycle(RemotingLifecycleEvent::Connected { authority, remote_system, .. }) => {
      assert_eq!(authority, &authority_for_a);
      assert_eq!(remote_system, "system-b");
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }

  let node_a = RemoteNodeId::new("system-a", "loopback-a.local", Some(4100), 11);
  let result_b = manager_b.handle(EndpointManagerCommand::HandshakeAccepted {
    authority:   authority_for_b.clone(),
    remote_node: node_a,
    now:         3,
  });
  assert_eq!(result_b.effects.len(), 2);
  match &result_b.effects[0] {
    | EndpointManagerEffect::DeliverEnvelopes { envelopes, .. } => {
      assert_eq!(envelopes, &vec![envelope("b->a")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
  match &result_b.effects[1] {
    | EndpointManagerEffect::Lifecycle(RemotingLifecycleEvent::Connected { authority, remote_system, .. }) => {
      assert_eq!(authority, &authority_for_b);
      assert_eq!(remote_system, "system-a");
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }
}

#[test]
fn loopback_quarantine_manual_override_flow_emits_events() {
  let loopback = LoopbackPair::new();
  let mgr = manager();
  let authority = loopback.authority_for_manager_a();
  mgr.handle(EndpointManagerCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointManagerCommand::EnqueueDeferred { authority: authority.clone(), envelope: envelope("pending") });

  let reason = QuarantineReason::new("uid mismatch");
  let quarantine = mgr.handle(EndpointManagerCommand::Quarantine {
    authority: authority.clone(),
    reason:    reason.clone(),
    resume_at: Some(200),
    now:       2,
  });
  assert_eq!(quarantine.effects.len(), 2);
  match &quarantine.effects[0] {
    | EndpointManagerEffect::DiscardDeferred { envelopes, .. } => assert_eq!(envelopes, &vec![envelope("pending")]),
    | other => panic!("unexpected discard effect: {other:?}"),
  }
  match &quarantine.effects[1] {
    | EndpointManagerEffect::Lifecycle(RemotingLifecycleEvent::Quarantined {
      authority: quarantined,
      reason: msg,
      ..
    }) => {
      assert_eq!(quarantined, &authority);
      assert_eq!(msg, reason.message());
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }

  mgr.handle(EndpointManagerCommand::EnqueueDeferred { authority: authority.clone(), envelope: envelope("retry") });

  let recover = mgr.handle(EndpointManagerCommand::Recover {
    authority: authority.clone(),
    endpoint:  Some(loopback.endpoint_to_manager_b()),
    now:       3,
  });
  assert!(matches!(recover.effects.as_slice(), [EndpointManagerEffect::StartHandshake { .. }]));

  let result = mgr.handle(EndpointManagerCommand::HandshakeAccepted {
    authority:   authority.clone(),
    remote_node: RemoteNodeId::new("system-b", "loopback-b.local", Some(4200), 99),
    now:         4,
  });
  assert_eq!(result.effects.len(), 2);
  match &result.effects[0] {
    | EndpointManagerEffect::DeliverEnvelopes { envelopes, .. } => assert_eq!(envelopes, &vec![envelope("retry")]),
    | other => panic!("unexpected deliver effect: {other:?}"),
  }
  match &result.effects[1] {
    | EndpointManagerEffect::Lifecycle(RemotingLifecycleEvent::Connected { authority: connected, .. }) => {
      assert_eq!(connected, &authority);
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }
}

#[test]
fn suspect_notification_via_gate_emits_lifecycle_event() {
  let loopback = LoopbackPair::new();
  let mgr = manager();
  let authority = loopback.authority_for_manager_a();
  mgr.handle(EndpointManagerCommand::RegisterInbound { authority: authority.clone(), now: 1 });

  let gate =
    mgr.handle(EndpointManagerCommand::Gate { authority: authority.clone(), resume_at: Some(999), now: 5 });
  assert_eq!(gate.effects.len(), 1);
  match &gate.effects[0] {
    | EndpointManagerEffect::Lifecycle(RemotingLifecycleEvent::Gated { authority: gated, correlation_id }) => {
      assert_eq!(gated, &authority);
      assert!(!correlation_id.is_nil());
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }
  assert_eq!(mgr.state(&authority), Some(AssociationState::Gated { resume_at: Some(999) }));
}
