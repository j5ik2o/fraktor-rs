use super::{EndpointManager, EndpointManagerCommand, EndpointManagerEffect};
use crate::core::{
  association_state::AssociationState, deferred_envelope::DeferredEnvelope, quarantine_reason::QuarantineReason,
  remote_node_id::RemoteNodeId, transport::TransportEndpoint,
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
  assert!(result.effects.is_empty());
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
  assert_eq!(result.effects, vec![EndpointManagerEffect::DeliverEnvelopes {
    authority: authority.clone(),
    envelopes: vec![envelope("m1")],
  }]);

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

  assert_eq!(result.effects, vec![EndpointManagerEffect::DiscardDeferred {
    authority: authority.clone(),
    reason:    reason.clone(),
    envelopes: vec![envelope("m1")],
  }]);

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
  assert_eq!(result.effects, vec![EndpointManagerEffect::DeliverEnvelopes {
    authority,
    envelopes: vec![envelope("m2")],
  }]);
}
