#![cfg(feature = "std")]

use alloc::boxed::Box;
use core::convert::TryFrom;

use fraktor_actor_rs::core::{
  actor::actor_path::{ActorPath, ActorPathParts, GuardianKind},
  event::stream::{CorrelationId, RemotingLifecycleEvent},
  serialization::{SerializedMessage, SerializerId},
};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use super::{
  super::QuarantineReason, AssociationState, EndpointAssociationCommand, EndpointAssociationCoordinator,
  EndpointAssociationEffect,
};
use crate::core::{
  envelope::{DeferredEnvelope, OutboundPriority, RemotingEnvelope},
  remote_node_id::RemoteNodeId,
  transport::{LoopbackTransport, RemoteTransport, TransportBind, TransportEndpoint},
};

fn coordinator() -> EndpointAssociationCoordinator {
  EndpointAssociationCoordinator::new()
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
  let mut parts = ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520)));
  parts = parts.with_guardian(GuardianKind::User);
  let recipient = ActorPath::from_parts(parts).child("svc");
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 0);
  let serializer = SerializerId::try_from(41).expect("serializer id");
  let serialized = SerializedMessage::new(serializer, None, label.as_bytes().to_vec());
  let envelope =
    RemotingEnvelope::new(recipient, remote, None, serialized, CorrelationId::nil(), OutboundPriority::User);
  DeferredEnvelope::new(envelope)
}

struct LoopbackPair {
  _transport:  LoopbackTransport<StdToolbox>,
  authority_a: String,
  authority_b: String,
}

impl LoopbackPair {
  fn new() -> Self {
    let mut transport = LoopbackTransport::<StdToolbox>::default();
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

  fn authority_for_coordinator_a(&self) -> String {
    self.authority_b.clone()
  }

  fn authority_for_coordinator_b(&self) -> String {
    self.authority_a.clone()
  }

  fn endpoint_to_coordinator_a(&self) -> TransportEndpoint {
    TransportEndpoint::new(self.authority_a.clone())
  }

  fn endpoint_to_coordinator_b(&self) -> TransportEndpoint {
    TransportEndpoint::new(self.authority_b.clone())
  }
}

#[test]
fn register_and_handshake_transitions_states() {
  let mut mgr = coordinator();
  let register = EndpointAssociationCommand::RegisterInbound { authority: "loopback:4100".into(), now: 1 };
  let result = mgr.handle(register);
  assert!(result.effects.is_empty());

  let associate = EndpointAssociationCommand::Associate {
    authority: "loopback:4100".into(),
    endpoint:  sample_endpoint(),
    now:       2,
  };
  let result = mgr.handle(associate);
  assert_eq!(result.effects, vec![EndpointAssociationEffect::StartHandshake {
    authority: "loopback:4100".into(),
    endpoint:  sample_endpoint(),
  }]);

  let accept = EndpointAssociationCommand::HandshakeAccepted {
    authority:   "loopback:4100".into(),
    remote_node: sample_remote(),
    now:         3,
  };
  let result = mgr.handle(accept);
  assert_eq!(result.effects.len(), 1);
  match &result.effects[0] {
    | EndpointAssociationEffect::Lifecycle(event) => match event {
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
  let mut mgr = coordinator();
  let authority = "loopback:4200".to_string();
  mgr.handle(EndpointAssociationCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointAssociationCommand::Associate {
    authority: authority.clone(),
    endpoint:  sample_endpoint(),
    now:       2,
  });

  let enqueue =
    EndpointAssociationCommand::EnqueueDeferred { authority: authority.clone(), envelope: Box::new(envelope("m1")) };
  let result = mgr.handle(enqueue);
  assert!(result.effects.is_empty());

  let result = mgr.handle(EndpointAssociationCommand::HandshakeAccepted {
    authority:   authority.clone(),
    remote_node: sample_remote(),
    now:         3,
  });
  assert_eq!(result.effects.len(), 2);
  match &result.effects[0] {
    | EndpointAssociationEffect::DeliverEnvelopes { authority: deliver_authority, envelopes } => {
      assert_eq!(deliver_authority, &authority);
      assert_eq!(envelopes, &vec![envelope("m1")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
  match &result.effects[1] {
    | EndpointAssociationEffect::Lifecycle(event) => match event {
      | RemotingLifecycleEvent::Connected { authority: connected_authority, .. } => {
        assert_eq!(connected_authority, &authority);
      },
      | other => panic!("unexpected lifecycle event: {other:?}"),
    },
    | other => panic!("unexpected effect: {other:?}"),
  }

  let immediate = mgr.handle(EndpointAssociationCommand::EnqueueDeferred {
    authority: authority.clone(),
    envelope:  Box::new(envelope("m2")),
  });
  assert_eq!(immediate.effects, vec![EndpointAssociationEffect::DeliverEnvelopes {
    authority,
    envelopes: vec![envelope("m2")],
  }]);
}

#[test]
fn quarantine_discards_deferred_messages() {
  let mut mgr = coordinator();
  let authority = "loopback:4300".to_string();
  mgr.handle(EndpointAssociationCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointAssociationCommand::Associate {
    authority: authority.clone(),
    endpoint:  sample_endpoint(),
    now:       2,
  });
  mgr.handle(EndpointAssociationCommand::EnqueueDeferred {
    authority: authority.clone(),
    envelope:  Box::new(envelope("m1")),
  });

  let reason = QuarantineReason::new("uid mismatch");
  let result = mgr.handle(EndpointAssociationCommand::Quarantine {
    authority: authority.clone(),
    reason:    reason.clone(),
    resume_at: Some(50),
    now:       3,
  });

  assert_eq!(result.effects.len(), 2);
  match &result.effects[0] {
    | EndpointAssociationEffect::DiscardDeferred {
      authority: discard_authority,
      reason: discard_reason,
      envelopes,
    } => {
      assert_eq!(discard_authority, &authority);
      assert_eq!(discard_reason, &reason);
      assert_eq!(envelopes, &vec![envelope("m1")]);
    },
    | other => panic!("unexpected discard effect: {other:?}"),
  }
  match &result.effects[1] {
    | EndpointAssociationEffect::Lifecycle(event) => match event {
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
fn enqueue_deferred_while_quarantined_is_discarded() {
  let mut mgr = coordinator();
  let authority = "loopback:4301".to_string();
  mgr.handle(EndpointAssociationCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  let reason = QuarantineReason::new("uid mismatch");
  mgr.handle(EndpointAssociationCommand::Quarantine {
    authority: authority.clone(),
    reason:    reason.clone(),
    resume_at: Some(80),
    now:       2,
  });

  let result = mgr.handle(EndpointAssociationCommand::EnqueueDeferred {
    authority: authority.clone(),
    envelope:  Box::new(envelope("drop-me")),
  });

  assert_eq!(result.effects.len(), 1);
  match &result.effects[0] {
    | EndpointAssociationEffect::DiscardDeferred {
      authority: discard_authority,
      reason: discard_reason,
      envelopes,
    } => {
      assert_eq!(discard_authority, &authority);
      assert_eq!(discard_reason, &reason);
      assert_eq!(envelopes, &vec![envelope("drop-me")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
}

#[test]
fn recover_from_quarantine_restarts_handshake() {
  let mut mgr = coordinator();
  let authority = "loopback:4400".to_string();
  mgr.handle(EndpointAssociationCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointAssociationCommand::Associate {
    authority: authority.clone(),
    endpoint:  sample_endpoint(),
    now:       2,
  });
  mgr.handle(EndpointAssociationCommand::Quarantine {
    authority: authority.clone(),
    reason:    QuarantineReason::new("network failure"),
    resume_at: None,
    now:       3,
  });

  let discarded = mgr.handle(EndpointAssociationCommand::EnqueueDeferred {
    authority: authority.clone(),
    envelope:  Box::new(envelope("m2")),
  });
  assert_eq!(discarded.effects.len(), 1);
  match &discarded.effects[0] {
    | EndpointAssociationEffect::DiscardDeferred { authority: discarded_authority, envelopes, .. } => {
      assert_eq!(discarded_authority, &authority);
      assert_eq!(envelopes, &vec![envelope("m2")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }

  let result = mgr.handle(EndpointAssociationCommand::Recover {
    authority: authority.clone(),
    endpoint:  Some(sample_endpoint_alt()),
    now:       4,
  });
  assert_eq!(result.effects, vec![EndpointAssociationEffect::StartHandshake {
    authority: authority.clone(),
    endpoint:  sample_endpoint_alt(),
  }]);

  let result = mgr.handle(EndpointAssociationCommand::HandshakeAccepted {
    authority:   authority.clone(),
    remote_node: sample_remote(),
    now:         5,
  });
  assert_eq!(result.effects.len(), 1);
  match &result.effects[0] {
    | EndpointAssociationEffect::Lifecycle(event) => match event {
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
  let mut coordinator_a = coordinator();
  let mut coordinator_b = coordinator();
  let authority_for_a = loopback.authority_for_coordinator_a();
  let authority_for_b = loopback.authority_for_coordinator_b();

  coordinator_a
    .handle(EndpointAssociationCommand::RegisterInbound { authority: authority_for_a.clone(), now: 1 });
  coordinator_b
    .handle(EndpointAssociationCommand::RegisterInbound { authority: authority_for_b.clone(), now: 1 });

  coordinator_a.handle(EndpointAssociationCommand::EnqueueDeferred {
    authority: authority_for_a.clone(),
    envelope:  Box::new(envelope("a->b")),
  });
  coordinator_b.handle(EndpointAssociationCommand::EnqueueDeferred {
    authority: authority_for_b.clone(),
    envelope:  Box::new(envelope("b->a")),
  });

  let handshake_a = coordinator_a.handle(EndpointAssociationCommand::Associate {
    authority: authority_for_a.clone(),
    endpoint:  loopback.endpoint_to_coordinator_b(),
    now:       2,
  });
  assert!(matches!(handshake_a.effects.as_slice(), [EndpointAssociationEffect::StartHandshake { .. }]));

  let handshake_b = coordinator_b.handle(EndpointAssociationCommand::Associate {
    authority: authority_for_b.clone(),
    endpoint:  loopback.endpoint_to_coordinator_a(),
    now:       2,
  });
  assert!(matches!(handshake_b.effects.as_slice(), [EndpointAssociationEffect::StartHandshake { .. }]));

  let node_b = RemoteNodeId::new("system-b", "loopback-b.local", Some(4200), 99);
  let result_a = coordinator_a.handle(EndpointAssociationCommand::HandshakeAccepted {
    authority:   authority_for_a.clone(),
    remote_node: node_b,
    now:         3,
  });
  assert_eq!(result_a.effects.len(), 2);
  match &result_a.effects[0] {
    | EndpointAssociationEffect::DeliverEnvelopes { envelopes, .. } => {
      assert_eq!(envelopes, &vec![envelope("a->b")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
  match &result_a.effects[1] {
    | EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Connected { authority, remote_system, .. }) => {
      assert_eq!(authority, &authority_for_a);
      assert_eq!(remote_system, "system-b");
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }

  let node_a = RemoteNodeId::new("system-a", "loopback-a.local", Some(4100), 11);
  let result_b = coordinator_b.handle(EndpointAssociationCommand::HandshakeAccepted {
    authority:   authority_for_b.clone(),
    remote_node: node_a,
    now:         3,
  });
  assert_eq!(result_b.effects.len(), 2);
  match &result_b.effects[0] {
    | EndpointAssociationEffect::DeliverEnvelopes { envelopes, .. } => {
      assert_eq!(envelopes, &vec![envelope("b->a")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
  match &result_b.effects[1] {
    | EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Connected { authority, remote_system, .. }) => {
      assert_eq!(authority, &authority_for_b);
      assert_eq!(remote_system, "system-a");
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }
}

#[test]
fn loopback_quarantine_manual_override_flow_emits_events() {
  let loopback = LoopbackPair::new();
  let mut mgr = coordinator();
  let authority = loopback.authority_for_coordinator_a();
  mgr.handle(EndpointAssociationCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointAssociationCommand::EnqueueDeferred {
    authority: authority.clone(),
    envelope:  Box::new(envelope("pending")),
  });

  let reason = QuarantineReason::new("uid mismatch");
  let quarantine = mgr.handle(EndpointAssociationCommand::Quarantine {
    authority: authority.clone(),
    reason:    reason.clone(),
    resume_at: Some(200),
    now:       2,
  });
  assert_eq!(quarantine.effects.len(), 2);
  match &quarantine.effects[0] {
    | EndpointAssociationEffect::DiscardDeferred { envelopes, .. } => {
      assert_eq!(envelopes, &vec![envelope("pending")])
    },
    | other => panic!("unexpected discard effect: {other:?}"),
  }
  match &quarantine.effects[1] {
    | EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Quarantined {
      authority: quarantined,
      reason: msg,
      ..
    }) => {
      assert_eq!(quarantined, &authority);
      assert_eq!(msg, reason.message());
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }

  let discarded = mgr.handle(EndpointAssociationCommand::EnqueueDeferred {
    authority: authority.clone(),
    envelope:  Box::new(envelope("retry")),
  });
  assert_eq!(discarded.effects.len(), 1);
  match &discarded.effects[0] {
    | EndpointAssociationEffect::DiscardDeferred { envelopes, .. } => {
      assert_eq!(envelopes, &vec![envelope("retry")]);
    },
    | other => panic!("unexpected discard effect: {other:?}"),
  }

  let recover = mgr.handle(EndpointAssociationCommand::Recover {
    authority: authority.clone(),
    endpoint:  Some(loopback.endpoint_to_coordinator_b()),
    now:       3,
  });
  assert!(matches!(recover.effects.as_slice(), [EndpointAssociationEffect::StartHandshake { .. }]));

  let result = mgr.handle(EndpointAssociationCommand::HandshakeAccepted {
    authority:   authority.clone(),
    remote_node: RemoteNodeId::new("system-b", "loopback-b.local", Some(4200), 99),
    now:         4,
  });
  assert_eq!(result.effects.len(), 1);
  match &result.effects[0] {
    | EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Connected { authority: connected, .. }) => {
      assert_eq!(connected, &authority);
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }
}

#[test]
fn suspect_notification_via_gate_emits_lifecycle_event() {
  let loopback = LoopbackPair::new();
  let mut mgr = coordinator();
  let authority = loopback.authority_for_coordinator_a();
  mgr.handle(EndpointAssociationCommand::RegisterInbound { authority: authority.clone(), now: 1 });

  let gate =
    mgr.handle(EndpointAssociationCommand::Gate { authority: authority.clone(), resume_at: Some(999), now: 5 });
  assert_eq!(gate.effects.len(), 1);
  match &gate.effects[0] {
    | EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Gated { authority: gated, correlation_id }) => {
      assert_eq!(gated, &authority);
      assert!(!correlation_id.is_nil());
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }
  assert_eq!(mgr.state(&authority), Some(AssociationState::Gated { resume_at: Some(999) }));
}

#[test]
fn handshake_timeout_moves_associating_to_gated_and_requires_recover() {
  let mut mgr = coordinator();
  let authority = "loopback:4500".to_string();
  mgr.handle(EndpointAssociationCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointAssociationCommand::Associate {
    authority: authority.clone(),
    endpoint:  sample_endpoint(),
    now:       2,
  });

  let timeout = mgr.handle(EndpointAssociationCommand::HandshakeTimedOut {
    authority: authority.clone(),
    resume_at: Some(20),
    now:       10,
  });
  assert_eq!(timeout.effects.len(), 1);
  match &timeout.effects[0] {
    | EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Gated { authority: gated, correlation_id }) => {
      assert_eq!(gated, &authority);
      assert!(!correlation_id.is_nil());
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }
  assert_eq!(mgr.state(&authority), Some(AssociationState::Gated { resume_at: Some(20) }));

  let ignored = mgr.handle(EndpointAssociationCommand::HandshakeAccepted {
    authority:   authority.clone(),
    remote_node: sample_remote(),
    now:         11,
  });
  assert!(ignored.effects.is_empty());
  assert_eq!(mgr.state(&authority), Some(AssociationState::Gated { resume_at: Some(20) }));

  let recover = mgr.handle(EndpointAssociationCommand::Recover {
    authority: authority.clone(),
    endpoint:  Some(sample_endpoint_alt()),
    now:       12,
  });
  assert_eq!(recover.effects, vec![EndpointAssociationEffect::StartHandshake {
    authority: authority.clone(),
    endpoint:  sample_endpoint_alt(),
  }]);

  let accepted = mgr.handle(EndpointAssociationCommand::HandshakeAccepted {
    authority:   authority.clone(),
    remote_node: sample_remote(),
    now:         13,
  });
  assert!(accepted.effects.iter().any(|effect| {
    matches!(
      effect,
      EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Connected {
        authority: connected_authority,
        ..
      }) if connected_authority == &authority
    )
  }));
}

#[test]
fn handshake_timeout_discards_deferred_messages() {
  let mut mgr = coordinator();
  let authority = "loopback:4501".to_string();
  mgr.handle(EndpointAssociationCommand::RegisterInbound { authority: authority.clone(), now: 1 });
  mgr.handle(EndpointAssociationCommand::Associate {
    authority: authority.clone(),
    endpoint:  sample_endpoint(),
    now:       2,
  });
  mgr.handle(EndpointAssociationCommand::EnqueueDeferred {
    authority: authority.clone(),
    envelope:  Box::new(envelope("queued-before-timeout")),
  });

  let timeout = mgr.handle(EndpointAssociationCommand::HandshakeTimedOut {
    authority: authority.clone(),
    resume_at: None,
    now:       3,
  });

  assert_eq!(timeout.effects.len(), 2);
  match &timeout.effects[0] {
    | EndpointAssociationEffect::DiscardDeferred { authority: discard_authority, reason, envelopes } => {
      assert_eq!(discard_authority, &authority);
      assert_eq!(reason.message(), "handshake timed out");
      assert_eq!(envelopes, &vec![envelope("queued-before-timeout")]);
    },
    | other => panic!("unexpected effect: {other:?}"),
  }
  match &timeout.effects[1] {
    | EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Gated { authority: gated, .. }) => {
      assert_eq!(gated, &authority);
    },
    | other => panic!("unexpected lifecycle effect: {other:?}"),
  }
  assert_eq!(mgr.state(&authority), Some(AssociationState::Gated { resume_at: None }));
}
