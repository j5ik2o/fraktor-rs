use super::RemotingLifecycleEvent;
use crate::core::event::stream::{CorrelationId, GracefulShutdownQuarantinedEvent, ThisActorSystemQuarantinedEvent};

#[test]
fn supports_graceful_shutdown_quarantined_variant() {
  let event = RemotingLifecycleEvent::GracefulShutdownQuarantined(GracefulShutdownQuarantinedEvent::new(
    "127.0.0.1:2552",
    7,
    "left",
  ));
  match event {
    | RemotingLifecycleEvent::GracefulShutdownQuarantined(payload) => {
      assert_eq!(payload.authority(), "127.0.0.1:2552");
      assert_eq!(payload.uid(), 7);
      assert_eq!(payload.reason(), "left");
    },
    | other => panic!("unexpected variant: {other:?}"),
  }
}

#[test]
fn supports_this_actor_system_quarantined_variant() {
  let event = RemotingLifecycleEvent::ThisActorSystemQuarantined(ThisActorSystemQuarantinedEvent::new(
    "127.0.0.1:2552",
    "127.0.0.1:2553",
  ));
  match event {
    | RemotingLifecycleEvent::ThisActorSystemQuarantined(payload) => {
      assert_eq!(payload.local_authority(), "127.0.0.1:2552");
      assert_eq!(payload.remote_authority(), "127.0.0.1:2553");
    },
    | other => panic!("unexpected variant: {other:?}"),
  }
}

#[test]
fn preserves_existing_variants() {
  let event = RemotingLifecycleEvent::Gated {
    authority:      "127.0.0.1:2559".into(),
    correlation_id: CorrelationId::from_u128(33),
  };
  assert!(matches!(
    event,
    RemotingLifecycleEvent::Gated {
      authority,
      correlation_id
    } if authority == "127.0.0.1:2559" && correlation_id == CorrelationId::from_u128(33)
  ));
}
